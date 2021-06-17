#![feature(hash_drain_filter)]

#[macro_use]
extern crate lazy_static;

mod cli;
mod log;
mod socket;

pub fn main() -> Result<(), std::io::Error> {
    let available_serial_ports = serialport::available_ports().unwrap_or_default();

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

    if cli::options().available_serial_ports_full {
        println!("Available serial ports:\n{:#?}", available_serial_ports);
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
    log!("UDP server: {}", socket_address);
    let socket = socket::new(&socket_address)
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
    loop {
        if let Ok(size) = serial.read(&mut serial_vector) {
            let data = serial_vector[..size].to_vec();
            if !data.is_empty() {
                log!("R {} : {:?}", serial_path, data);
                socket.write(&data);
            }
        }

        let data = socket.read();
        if !data.is_empty() {
            serial.write_all(&data)?;
        }

        // Avoid cpu spin lock
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
