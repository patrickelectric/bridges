#![feature(hash_drain_filter)]

use clap::{self, Clap};

use std::io::Read;
use std::path::PathBuf;

mod socket;

/// This doc string acts as a help message when the user runs '--help'
/// as do all doc strings on fields
#[derive(Clap, Debug)]
#[clap(version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Does serial<->UDP bridges, for now.")]
#[clap(setting = clap::AppSettings::ColoredHelp)]
struct Opts {
    /// Enable verbosity
    #[clap(short = 'v', long = "verbose")]
    verbose: bool,

    /// Enable ABR (Automatic Baud Rate) procedure
    #[clap(long = "abr")]
    automatic_baud_rate_procedure: bool,

    /// Sets the connection serial port and baud rate, default baud rate is 115200
    #[clap(
        short = 'p',
        long = "port",
        parse(from_os_str),
        value_name = "SERIAL_PORT:BAUD_RATE",
        required_unless_present_any = &["available-serial-ports", "available-serial-ports-full"],
    )]
    serial_port: Option<PathBuf>,

    /// Prints the available serial ports
    #[clap(long = "available-serial-ports")]
    available_serial_ports: bool,

    /// Prints the available serial ports and any possible information
    #[clap(long = "available-serial-ports-full")]
    available_serial_ports_full: bool,

    /// Sets the UDP connection ip and port
    #[clap(
        short = 'u',
        long = "udp-port",
        value_name = "IP:PORT",
        default_value = "0.0.0.0:9092"
    )]
    udp_address: String,
}

pub fn main() -> Result<(), std::io::Error> {
    let opts: Opts = Opts::parse();

    let available_serial_ports = serialport::available_ports().unwrap_or_default();

    if opts.available_serial_ports {
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

    if opts.available_serial_ports_full {
        println!("Available serial ports:\n{:#?}", available_serial_ports);
        return Ok(());
    }

    if !opts.serial_port.as_ref().unwrap().exists() {
        panic!(
            "Serial port does not exist: {}",
            opts.serial_port.unwrap().to_str().unwrap()
        );
    }

    let serial_arg_result = opts
        .serial_port
        .as_ref()
        .unwrap()
        .to_str()
        .unwrap()
        .split(':')
        .collect::<Vec<&str>>();

    let (serial_path, baud_rate) = (
        serial_arg_result[0],
        serial_arg_result
            .get(1)
            .unwrap_or(&"115200")
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("Argument should follow the format {}", "IP:PORT")),
    );

    let mut serial = serialport::new(serial_path, baud_rate)
        .open()
        .unwrap_or_else(|_| {
            panic!(
                "Failed to open port: {} with baudrate {}",
                serial_path, baud_rate
            )
        });

    let socket = socket::new(&opts.udp_address)
        .unwrap_or_else(|error| panic!("Failed to bind address: {}", error));

    // Serial and socket are ready, time to run ABR
    if opts.automatic_baud_rate_procedure {
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
