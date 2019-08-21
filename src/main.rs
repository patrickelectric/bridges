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
fn is_closed(state: Ready) -> bool {
    false
}

pub fn main() {
    let usb_arg_format = "SERIAL_PORT:BAUD_RATE";
    let matches = App::new("Bridges")
        .version("1.0")
        .about("Does serial<->UDP bridges, for now.")
        .author("Patrick J.P. <patrickelectric@gmail.com>")
        .arg(
            Arg::with_name("serial_port")
                .short("p")
                .long("port")
                .value_name(usb_arg_format)
                .help("Sets the connection serial port and baud rate")
                .takes_value(true)
                .default_value("/dev/ttyUSB0:2000000"),
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
            Arg::with_name("v")
                .short("v")
                .long("verbose")
                .help("Enables verbosity"),
        )
        .get_matches();

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

    println!("Server in {:?}", &socket.local_addr().ok());

    // Set buffer for serial and UDP
    let mut serial_buffer = [0u8; 4096];
    let mut udp_buffer = [0u8; 4096];

    // Hold the last UDP client, this is the default client that will receive the serial output
    let mut client = addr;

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
        if let Err(ref e) = poll.poll(&mut events, Some(time::Duration::from_micros(1))) {
            println!("poll failed: {}", e);
            break;
        }

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
                                Ok((_count, _client)) => {
                                    client = _client;
                                    rx.write_all(&udp_buffer[.._count])
                                        .expect("Failed to write in serial device.");
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
                                    socket
                                        .send_to(&serial_buffer[..count], &client)
                                        .expect("Failed to write for UDP client.");
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
