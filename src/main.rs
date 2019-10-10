extern crate clap;
extern crate mio;
extern crate mio_serial;

use clap::{App, Arg};

#[cfg(unix)]
use mio::unix::UnixReady;

use mio::net::UdpSocket;
use mio::{Events, Poll, PollOpt, Ready, Token};
use std::io::{self, Read, Write};
use std::{str, thread, time};

const SERIAL_TOKEN: Token = Token(1);
const UDP_TOKEN: Token = Token(0);

// ready_of_interest
#[cfg(unix)]
fn ready_of_interest() -> Ready {
    Ready::readable() | UnixReady::hup() | UnixReady::error()
}

#[cfg(windows)]
fn ready_of_interest() -> Ready {
    Ready::readable()
}

// is_closed
#[cfg(unix)]
fn is_closed(state: Ready) -> bool {
    state.contains(UnixReady::hup() | UnixReady::error())
}

#[cfg(windows)]
fn is_closed(_: Ready) -> bool {
    false
}

pub fn main() {
    let usb_arg_format = "SERIAL_PORT:BAUD_RATE";
    let available_serial_ports_name = "available_serial_ports";
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("Does serial<->UDP bridges, for now.")
        .author(env!("CARGO_PKG_AUTHORS"))
        .arg(
            Arg::with_name("serial_port")
                .short("p")
                .long("port")
                .value_name(usb_arg_format)
                .help("Sets the connection serial port and baud rate")
                .takes_value(true)
                .required_unless(available_serial_ports_name),
        )
        .arg(
            Arg::with_name("udp_port")
                .short("u")
                .long("UDP")
                .value_name("IP:PORT")
                .help("Sets the UDP connection ip and port")
                .takes_value(true)
                .default_value("0.0.0.0:9092"),
        )
        .arg(
            Arg::with_name("timeout")
                .short("t")
                .long("timeout")
                .value_name("TIMEOUT")
                .help("Set the timeout of UDP clients that does not communicate with server in milliseconds.")
                .takes_value(true)
                .default_value("10000"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enables verbosity"),
        )
        .arg(
            Arg::with_name(available_serial_ports_name)
                .long("available-serial-ports")
                .help("Prints the available serial ports."),
        )
        .get_matches();

    // Check verbose mode
    let verbose = matches.is_present("verbose");

    if matches.is_present("available_serial_ports") {
        println!("{:#?}", mio_serial::available_ports().unwrap());
        return;
    }

    // Set timeout
    let timeout: u128 = matches
        .value_of("timeout")
        .unwrap()
        .parse::<u128>()
        .unwrap_or_else(|_| panic!("Expected u128 value got {:?}", matches.value_of("timeout")));

    // Configure serial port
    let serial_arg_result = matches
        .value_of("serial_port")
        .unwrap()
        .split(':')
        .collect::<Vec<&str>>();
    let (tty_path, baud_rate) = (
        serial_arg_result[0],
        serial_arg_result[1]
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("Argument should follow the format {}", usb_arg_format)),
    );

    let mut settings = mio_serial::SerialPortSettings::default();
    settings.baud_rate = baud_rate;

    println!("Opening {} with {:#?}", tty_path, settings);
    let mut rx = mio_serial::Serial::from_path(&tty_path, &settings)
        .expect("Something is wrong with serial path or baud rate.");

    // TODO: Need to figure out how to wait for port to be open
    thread::sleep(time::Duration::from_millis(100));

    // Configure UDP connection
    let addr: std::net::SocketAddr = matches
        .value_of("udp_port")
        .unwrap()
        .parse()
        .expect("Unable to parse socket address");
    let socket = UdpSocket::bind(&addr).expect("It was not possible to do UDP bind.");

    println!("Server in {}", &socket.local_addr().unwrap());

    // Set buffer for serial and UDP
    let mut serial_buffer = [0u8; 4096];
    let mut udp_buffer = [0u8; 4096];

    // Hold a list of clients with timeouts
    let mut clients = std::collections::HashMap::<std::net::SocketAddr, u128>::new();
    // Get the time to calculate timeout of the clients
    let now = std::time::Instant::now();

    // Configure poll and events
    let poll = Poll::new().expect("Failed to create poll.");
    let mut events = Events::with_capacity(4096);

    poll.register(&rx, SERIAL_TOKEN, ready_of_interest(), PollOpt::edge())
        .expect("Failed to register in poll.");
    poll.register(&socket, UDP_TOKEN, ready_of_interest(), PollOpt::edge())
        .expect("Failed to register in poll.");

    println!("Running!");

    // Loop inside `is_readable` check is necessary.
    // With edge triggered events, we must perform reading until we receive a WouldBlock.
    // Check https://docs.rs/mio/0.6/mio/struct.Poll.html for details.
    'outer: loop {
        if let Err(ref e) = poll.poll(&mut events, None) {
            println!("Poll failed: {}", e);
            break;
        }

        // Elapsed time to calculate clients timeout
        let elapsed_time_ms = now.elapsed().as_millis();

        for event in events.iter() {
            match event.token() {
                UDP_TOKEN => {
                    let ready = event.readiness();
                    if is_closed(ready) {
                        println!("Quitting due to event: {:?}", ready);
                        break 'outer;
                    }
                    if ready.is_readable() {
                        loop {
                            match socket.recv_from(&mut udp_buffer) {
                                Ok((_count, client)) => {
                                    clients.insert(client, elapsed_time_ms);

                                    if verbose {
                                        println!("< {}: {:?}", client.ip(), &udp_buffer[.._count]);
                                        println!(
                                            "> {:?}: {:?}",
                                            mio_serial::SerialPort::name(&rx).unwrap(),
                                            &udp_buffer[.._count]
                                        );
                                    }
                                    match rx.write_all(&udp_buffer[.._count]) {
                                        Err(e) => {
                                            println!("Error while writing in serial: {:?}", e)
                                        }
                                        Ok(_) => (),
                                    };
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(ref e) => {
                                    println!("Quitting due to read error: {}", e);
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                SERIAL_TOKEN => {
                    let ready = event.readiness();
                    if is_closed(ready) {
                        println!("Quitting due to event: {:?}", ready);
                        break 'outer;
                    }
                    if ready.is_readable() {
                        loop {
                            match rx.read(&mut serial_buffer) {
                                Ok(count) => {
                                    if verbose {
                                        println!(
                                            "< {:?}: {:?}",
                                            mio_serial::SerialPort::name(&rx).unwrap(),
                                            &serial_buffer[..count]
                                        );
                                    }
                                    clients.retain(|_, last_time| {
                                        elapsed_time_ms - *last_time < timeout
                                    });
                                    for (client, _) in &clients {
                                        if verbose {
                                            println!(
                                                "> {}: {:?}",
                                                client.ip(),
                                                &serial_buffer[..count]
                                            );
                                        }
                                        match socket.send_to(&serial_buffer[..count], &client) {
                                            Err(e) => {
                                                println!("Error while writing in UDP: {:?}", e)
                                            }
                                            Ok(_) => (),
                                        };
                                    }
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(ref e) => {
                                    println!("Quitting due to read error: {}", e);
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
    }
}
