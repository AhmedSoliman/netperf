use crate::common::control::TestResults;
use crate::common::data::Direction;
use colored::Colorize;

// Bytes
const KBYTES: usize = 1024;
const MBYTES: usize = 1024 * KBYTES;
const GBYTES: usize = 1024 * MBYTES;
const TBYTES: usize = 1024 * GBYTES;

// Bitrates
const KBITS: usize = 1000;
const MBITS: usize = 1000 * KBITS;
const GBITS: usize = 1000 * MBITS;
const TBITS: usize = 1000 * GBITS;

pub fn print_header() {
    println!("[ ID]   Interval          Transfer      Bitrate");
}
pub fn print_server_banner(port: u16) {
    println!("--------------------------------------");
    println!("{} {}", "Listening on port".cyan(), port);
    println!("--------------------------------------");
}

pub fn humanize_bytes(bytes: usize) -> String {
    if bytes < KBYTES {
        format!("{} B", bytes)
    } else if bytes < MBYTES {
        format!("{:.2} KiB", bytes as f64 / KBYTES as f64)
    } else if bytes < GBYTES {
        format!("{:.2} MiB", bytes as f64 / MBYTES as f64)
    } else if bytes < TBYTES {
        format!("{:.2} GiB", bytes as f64 / GBYTES as f64)
    } else {
        format!("{:.2} TiB", bytes as f64 / TBYTES as f64)
    }
}

pub fn humanize_bitrate(bytes: usize, duration_millis: u64) -> String {
    // For higher accuracy we are getting the actual millis of the duration rather than the
    // rounded seconds.
    let bits = bytes * 8;
    // rate as fraction in seconds;
    let rate = (bits as f64 / duration_millis as f64) * 1000f64;
    if rate < KBITS as f64 {
        format!("{} Bits/sec", rate)
    } else if bytes < MBITS {
        format!("{:.2} Kbits/sec", rate / KBITS as f64)
    } else if bytes < GBITS {
        format!("{:.2} Mbits/sec", rate / MBITS as f64)
    } else if bytes < TBITS {
        format!("{:.2} Gbits/sec", rate / GBITS as f64)
    } else {
        format!("{:.2} Tbits/sec", rate / TBITS as f64)
    }
}

pub fn print_stats(
    id: Option<usize>,
    offset_from_start_millis: u64,
    duration_millis: u64,
    bytes_transferred: usize,
    sender: bool,
    _syscalls: usize,
    _block_size: usize,
) {
    let end_point = offset_from_start_millis + duration_millis;
    // Calculating the percentage of
    println!(
        "[{:>3}]   {:.2}..{:.2} sec  {}   {}        {}",
        id.map(|x| x.to_string())
            .unwrap_or_else(|| "SUM".to_owned()),
        offset_from_start_millis as f64 / 1000f64,
        end_point as f64 / 1000f64,
        humanize_bytes(bytes_transferred),
        humanize_bitrate(bytes_transferred, duration_millis),
        if sender {
            "sender".yellow()
        } else {
            "receiver".magenta()
        },
    );
}

pub fn print_summary(
    local_results: &TestResults,
    remote_results: &TestResults,
    direction: &Direction,
) {
    println!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - -");
    print_summary_header();
    // Streams IDs match between server and client (they make pairs of sender/receiver)
    // We print each stream sender then receiver. Then the sum of all senders and receivers.
    let mut sender_duration_millis = 0;
    let mut receiver_duration_millis = 0;
    let mut sender_bytes_transferred = 0;
    let mut receiver_bytes_transferred = 0;
    for (id, local_stats) in &local_results.streams {
        print_stats(
            Some(*id),
            0,
            local_stats.duration_millis,
            local_stats.bytes_transferred,
            local_stats.sender,
            local_stats.syscalls,
            0,
        );
        if local_stats.sender {
            sender_bytes_transferred += local_stats.bytes_transferred;
            sender_duration_millis =
                std::cmp::max(sender_duration_millis, local_stats.duration_millis);
        } else {
            receiver_bytes_transferred += local_stats.bytes_transferred;
            receiver_duration_millis =
                std::cmp::max(receiver_duration_millis, local_stats.duration_millis);
        }
        // find the remote counterpart. This is only valuable if we are not using
        // bidirectional streams. In bidirectional streams we already have the sender
        // and receiving data.
        if *direction != Direction::Bidirectional {
            if let Some(remote_stats) = remote_results.streams.get(id) {
                print_stats(
                    Some(*id),
                    0,
                    remote_stats.duration_millis,
                    remote_stats.bytes_transferred,
                    remote_stats.sender,
                    remote_stats.syscalls,
                    0,
                );
                if remote_stats.sender {
                    sender_bytes_transferred += remote_stats.bytes_transferred;
                    sender_duration_millis =
                        std::cmp::max(sender_duration_millis, remote_stats.duration_millis);
                } else {
                    receiver_bytes_transferred += remote_stats.bytes_transferred;
                    receiver_duration_millis =
                        std::cmp::max(receiver_duration_millis, remote_stats.duration_millis);
                }
            }
        }
    }
    // if we have more than one stream, let's print a SUM entry as well.
    if local_results.streams.len() > 1 {
        println!();
        print_stats(
            None,
            0,
            sender_duration_millis,
            sender_bytes_transferred,
            true,
            0,
            0,
        );
        print_stats(
            None,
            0,
            receiver_duration_millis,
            receiver_bytes_transferred,
            false,
            0,
            0,
        );
    }
}

fn print_summary_header() {
    println!(
        "{}   {}          {}      {}",
        "ID".bold(),
        "Interval".bold(),
        "Transfer".bold(),
        "Bitrate".bold()
    );
}
