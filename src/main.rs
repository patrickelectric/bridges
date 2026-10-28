use std::pin::pin;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    time::sleep,
};
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};

#[macro_use]
extern crate lazy_static;

mod cli;
mod log;
mod socket;

enum SerialCommand {
    Write(Vec<u8>),
    LineBreak,
}

async fn line_break(serial: &mut SerialStream) -> Result<(), std::io::Error> {
    serial.set_break()?;
    sleep(std::time::Duration::from_millis(10)).await;
    serial.clear_break()?;
    sleep(std::time::Duration::from_micros(10)).await;
    serial.write_all(&[b'U'; 10]).await
}

#[tokio::main]
pub async fn main() -> Result<(), std::io::Error> {
    let available_serial_ports = tokio_serial::available_ports().unwrap_or_default();

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
    let mut serial = tokio_serial::new(serial_path, baud_rate)
        .open_native_async()
        .unwrap_or_else(|_| {
            panic!(
                "Failed to open port: {} with baudrate {}",
                serial_path, baud_rate
            )
        });

    let socket_address = &cli::options().udp_address;
    let listen_port = cli::options().udp_listen_port.unwrap_or(0);
    let socket = socket::new(socket_address, listen_port)
        .await
        .unwrap_or_else(|error| panic!("Failed to bind address: {}", error));

    // Serial and socket are ready, time to run ABR
    if cli::options().automatic_baud_rate_procedure {
        log!("Start ABR procedure");
        line_break(&mut serial).await?;
    }

    let is_verbose = cli::is_verbose();
    let (command_tx, mut command_rx) = mpsc::channel::<SerialCommand>(32);

    let serial_fut = pin!(async {
        let mut serial_vector = vec![0; 4096];
        loop {
            tokio::select! {
                result = serial.read(&mut serial_vector) => match result {
                    Ok(size) => {
                        let data = &serial_vector[..size];
                        if !data.is_empty() {
                            if is_verbose {
                                log!("R {} : {:?}", serial_path, data);
                            }
                            socket.write(data).await;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                        panic!("Port disconnected: {serial_path}")
                    }
                    Err(_) => {}
                },
                Some(command) = command_rx.recv() => match command {
                    SerialCommand::Write(data) => {
                        if let Err(error) = serial.write_all(&data).await {
                            match error.kind() {
                                std::io::ErrorKind::TimedOut => {
                                    log!("Timeout error while writing to serial port. Consider increasing the baud rate on that port.");
                                }
                                _ => {
                                    log!("Error while writing to serial port: {}", error);
                                }
                            }
                        }
                    }
                    SerialCommand::LineBreak => {
                        log!("Start line break procedure");
                        line_break(&mut serial).await?;
                    }
                },
            }
        }
    });

    let udp_fut = pin!(async {
        loop {
            let (data, empty_datagram) = socket.read().await;
            if !data.is_empty() && command_tx.send(SerialCommand::Write(data)).await.is_err() {
                return Ok(());
            }
            if empty_datagram && command_tx.send(SerialCommand::LineBreak).await.is_err() {
                return Ok(());
            }
        }
    });

    tokio::select! {
        result = serial_fut => result,
        result = udp_fut => result,
    }
}
