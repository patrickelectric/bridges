#[macro_use]
extern crate lazy_static;

mod cli;
mod log;
mod socket;

use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_serial::SerialPort;
use tokio_serial::SerialPortBuilderExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let available_serial_ports = serialport::available_ports().unwrap_or_default();
    if cli::options().available_serial_ports_full {
        println!("Available serial ports:\n{:#?}", available_serial_ports);
        return Ok(());
    }
    if cli::options().available_serial_ports {
        println!(
            "Available serial ports: {}",
            available_serial_ports
                .iter()
                .map(|port| port.port_name.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );
        return Ok(());
    }

    let (serial_path, baud_rate) = cli::serial_port_configuration();
    log!("Serial port: {} with baud rate {}", serial_path, baud_rate);
    let mut serial = tokio_serial::new(serial_path.clone(), baud_rate)
        .open_native_async()
        .unwrap_or_else(|_| {
            panic!(
                "Failed to open port: {} with baudrate {}",
                serial_path, baud_rate
            )
        });

    let listen_port = cli::options().udp_listen_port.unwrap_or(0);
    let bind_addr = format!("0.0.0.0:{}", listen_port);
    let socket = UdpSocket::bind(&bind_addr)
        .await
        .unwrap_or_else(|error| panic!("Failed to bind UDP socket: {}", error));
    let remote_addr = cli::options().udp_address.clone();
    if let Err(e) = socket.connect(&remote_addr).await {
        log!("Failed to connect to remote address {}: {}", remote_addr, e);
        return Err(e.into());
    }

    if cli::options().automatic_baud_rate_procedure {
        log!("Start ABR procedure");
        serial.set_break()?;
        sleep(Duration::from_millis(10)).await;
        serial.clear_break()?;
        sleep(Duration::from_micros(10)).await;
        serial.write_all(&[b'U'; 10]).await?;
    }

    let (serial_tx, mut serial_rx) = mpsc::channel::<Vec<u8>>(2048);
    let (socket_tx, mut socket_rx) = mpsc::channel::<Vec<u8>>(2048);
    let is_verbose = cli::is_verbose();

    // Create a serial port for ABR if needed (separate from main I/O)
    let abr_serial = serial
        .try_clone()
        .expect("Failed to clone serial port for ABR procedure");

    // Serial read task
    let serial_path_clone = serial_path.clone();
    let mut serial_reader = serial
        .try_clone()
        .expect("Failed to clone serial port for reading");
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match serial_reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let data = buf[..n].to_vec();
                    if is_verbose {
                        log!("R {} : {:?}", serial_path_clone, data);
                    }
                    if serial_tx.send(data).await.is_err() {
                        log!("Serial channel closed, exiting read task");
                        break;
                    }
                }
                Ok(_) => {
                    // Empty read, continue
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        log!("Port disconnected: {}", serial_path_clone);
                        break;
                    }
                    log!("Serial read error: {}", e);
                }
            }
        }
    });

    // Socket read task: wait for datagram
    // Create a second socket for reading
    let socket_addr = socket.local_addr()?;
    let socket_clone = UdpSocket::bind(socket_addr).await?;
    socket_clone.connect(&remote_addr).await?;

    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match socket_clone.recv(&mut buf).await {
                Ok(n) if n > 0 => {
                    let data = buf[..n].to_vec();
                    if is_verbose {
                        log!("Received {} bytes from socket", n);
                    }
                    if socket_tx.send(data).await.is_err() {
                        log!("Socket channel closed, exiting read task");
                        break;
                    }
                }
                Ok(0) => {
                    // Received empty datagram, perform ABR procedure
                    log!("Received empty datagram, performing ABR procedure");
                    if let Err(e) = abr_serial.set_break() {
                        log!("Error setting break: {}", e);
                    }
                    sleep(Duration::from_millis(10)).await;
                    if let Err(e) = abr_serial.clear_break() {
                        log!("Error clearing break: {}", e);
                    }
                    sleep(Duration::from_micros(10)).await;
                }
                Ok(_) => {
                    // Handle other n values (n >= 1)
                    log!("Received unexpected datagram size");
                }
                Err(e) => {
                    log!("Socket read error: {}", e);
                }
            }
        }
    });

    // Socket write task: forward serial data to socket
    let socket_writer = socket;
    tokio::spawn(async move {
        while let Some(data) = serial_rx.recv().await {
            if let Err(e) = socket_writer.send(&data).await {
                log!("Socket send error: {}", e);
            }
        }
    });

    // Serial write task: forward socket data to serial
    let mut serial_writer = serial;
    tokio::spawn(async move {
        while let Some(data) = socket_rx.recv().await {
            if is_verbose {
                log!("Writing {} bytes to serial", data.len());
            }
            if let Err(e) = serial_writer.write_all(&data).await {
                match e.kind() {
                    std::io::ErrorKind::TimedOut => {
                        log!("Timeout writing to serial. Consider increasing its baud rate.");
                    }
                    _ => {
                        log!("Error writing to serial: {}", e);
                    }
                }
            }
        }
    });

    // Add signal handler for graceful shutdown or run indefinitely
    if let Ok(()) = tokio::signal::ctrl_c().await {
        log!("Received shutdown signal, exiting...");
    } else {
        futures::future::pending::<()>().await;
    }

    Ok(())
}
