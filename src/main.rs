// TODO: compile time check to detect when 'hash_drain_filter' has gone stable
//
// The following nightly feature was being used in a single line of code, so
// disabling it allow us to switch to stable Rust. We'd like to have a compile
// time chechk that would tell us when it's not nightly anymore.
//
// However it seems Rust currently lacks the ability to identify if a certain
// path is present during compile time without spawning a new build process (see
// https://rust-lang.github.io/rfcs/2523-cfg-path-version.html).
//
// Once the 'cfg_accessible' feature is implemented in Rust, a simple attribute
// check will allow us to implement this todo.
//
//#![feature(hash_drain_filter)]

#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "linux")]
mod udev;

mod cli;
mod log;
mod socket;

pub fn main() -> Result<(), std::io::Error> {
    let available_serial_ports = serialport::available_ports().unwrap_or_default();

    if cli::options().available_serial_ports_full {
        println!("{available_serial_ports:#?}");

        #[cfg(target_os = "linux")]
        for device in available_serial_ports {
            println!("port: {} -> {:?}", device.port_name, udev::get_device_links(&device.port_name).unwrap());
        }

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
        match serial.read(&mut serial_vector) {
            Ok(size) => {
                let data = serial_vector[..size].to_vec();
                if !data.is_empty() {
                    log!("R {} : {:?}", serial_path, data);
                    socket.write(&data);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                panic!("Port disconnected: {serial_path}")
            }
            Err(_) => {}
        }

        let data = socket.read();
        if !data.is_empty() {
            serial.write_all(&data)?;
        }

        // Avoid cpu spin lock
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
