use clap::Parser;

/// This doc string acts as a help message when the user runs '--help'
/// as do all doc strings on fields
#[derive(Parser, Debug)]
#[clap(version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Does serial<->UDP bridges, for now.")]
pub struct Options {
    /// Enable verbosity
    #[clap(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Enable ABR (Automatic Baud Rate) procedure
    #[clap(long = "abr")]
    pub automatic_baud_rate_procedure: bool,

    /// Disable UDP timeout disconnection (10s)
    #[clap(long = "no-udp-disconnection")]
    pub no_udp_disconnection: bool,

    /// Sets the connection serial port and baud rate, default baud rate is 115200
    #[clap(
        short = 'p',
        long = "port",
        value_name = "SERIAL_PORT:BAUD_RATE",
        required_unless_present_any = &["available-serial-ports", "available-serial-ports-full"],
    )]
    pub serial_port: Option<String>,

    /// Prints the available serial ports
    #[clap(long = "available-serial-ports")]
    pub available_serial_ports: bool,

    /// Prints the available serial ports and any possible information
    #[clap(long = "available-serial-ports-full")]
    pub available_serial_ports_full: bool,

    /// Sets the UDP connection ip and port
    #[clap(
        short = 'u',
        long = "udp-port",
        value_name = "IP:PORT",
        default_value = "0.0.0.0:9092"
    )]
    pub udp_address: String,
}

#[derive(Debug)]
struct Manager {
    options: Options,
}

lazy_static! {
    static ref MANAGER: std::sync::Arc<Manager> = std::sync::Arc::new(Manager::new());
}

impl Manager {
    fn new() -> Self {
        Self {
            options: Options::parse(),
        }
    }
}

// Check if the verbosity parameter was used
pub fn is_verbose() -> bool {
    return MANAGER.as_ref().options.verbose;
}

pub fn options<'a>() -> &'a Options {
    return &MANAGER.as_ref().options;
}

pub fn serial_port_configuration() -> (&'static str, u32) {
    let serial_arg_result = options()
        .serial_port
        .as_ref()
        .unwrap()
        .split(':')
        .collect::<Vec<&str>>();

    (
        serial_arg_result[0],
        serial_arg_result
            .get(1)
            .unwrap_or(&"115200")
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("Argument should follow the format {}", "IP:PORT")),
    )
}
