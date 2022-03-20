use clap::{ArgGroup, Parser};

#[derive(Debug, Parser)]
pub struct ServerOpts {
    /// Run in server mode
    #[clap(short, long, group = "server_or_client")]
    pub server: bool,
    /// Run in server mode
    #[clap(short = '1', long, group = "server_or_client")]
    pub one_off: bool,
}

#[derive(Debug, Parser)]
pub struct ClientOpts {
    /// Run in client mode, connect to <host>
    #[clap(short, long, group = "server_or_client")]
    pub client: Option<String>,
    /// Length of buffer to read or write, default is 2MiB (2097152)
    #[clap(short, long)]
    pub length: Option<usize>,
    /// SO_SNDBUF/SO_RECVBUF for the data streams, uses the system default if unset.
    #[clap(long)]
    pub socket_buffers: Option<usize>,
    /// Time in seconds to transmit for
    #[clap(short, long, default_value = "10")]
    pub time: u64,
    /// How many parallel streams used in testing
    #[clap(short = 'P', long, default_value = "1")]
    pub parallel: u16,
    /// Run in bidirectional mode. Client and server send and receive data.
    #[clap(long, group = "direction")]
    pub bidir: bool,
    /// Run in reverse mode (server sends, client receives)
    #[clap(short = 'R', long, group = "direction")]
    pub reverse: bool,
    /// Set TCP no delay, disabling Nagle's Algorithm
    #[clap(short = 'N', long)]
    pub no_delay: bool,
}

#[derive(Debug, Parser)]
pub struct CommonOpts {
    /// Server port to listen on/connect to
    #[clap(short, long, default_value = "7559")]
    pub port: u16,
    /// Seconds between periodic throughput reports
    #[clap(short, long, default_value = "1")]
    pub interval: u16,
}

#[derive(Debug, Parser)]
#[clap(
    name = "netperf", 
    groups = [
    ArgGroup::new("server_or_client").required(true),
    ArgGroup::new("direction")])
]
/// A network performance measurement tool
pub struct Opts {
    #[clap(flatten)]
    pub server_opts: ServerOpts,
    #[clap(flatten)]
    pub client_opts: ClientOpts,
    #[clap(flatten)]
    pub common_opts: CommonOpts,
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}
