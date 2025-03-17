#[macro_use]
extern crate lazy_static;

mod cli;
mod log;
mod socket;

pub fn main() -> Result<(), std::io::Error> {
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
    let mut serial = serialport::new(serial_path, baud_rate)
        .open()
        .unwrap_or_else(|_| {
            panic!(
                "Failed to open port: {} with baudrate {}",
                serial_path, baud_rate
            )
        });

    let socket_address = &cli::options().udp_address;
    let listen_port = cli::options().udp_listen_port.unwrap_or(0);
    let socket = socket::new(socket_address, listen_port)
        .unwrap_or_else(|error| panic!("Failed to bind address: {}", error));

    // Serial and socket are ready, time to run ABR
    if cli::options().automatic_baud_rate_procedure {
        log!("Start ABR procedure");
        serial.set_break()?;
        std::thread::sleep(std::time::Duration::from_millis(10));
        serial.clear_break()?;
        std::thread::sleep(std::time::Duration::from_micros(10));
        serial.write_all(&[b'U'; 10])?;
    }

    let mut serial_vector = vec![0; 4096];
    let is_verbose = cli::is_verbose();
    loop {
        match serial.read(&mut serial_vector) {
            Ok(size) => {
                let data = serial_vector[..size].to_vec();
                if !data.is_empty() {
                    if is_verbose {
                        log!("R {} : {:?}", serial_path, data);
                    }
                    socket.write(&data);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                panic!("Port disconnected: {serial_path}")
            }
            Err(_) => {}
        }

        let (data, empty_datagram) = socket.read();
        if !data.is_empty() {
            if let Err(error) = serial.write_all(&data) {
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

        if empty_datagram {
            log!("Start line break procedure");
            serial.set_break()?;
            std::thread::sleep(std::time::Duration::from_millis(10));
            serial.clear_break()?;
            std::thread::sleep(std::time::Duration::from_micros(10));
            serial.write_all(&[b'U'; 10])?;
        }

        // Avoid cpu spin lock
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
