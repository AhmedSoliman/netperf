# netperf
![](https://github.com/AhmedSoliman/netperf/workflows/Continuous%20Integration/badge.svg)
![https://crates.io/crates/netperf](https://img.shields.io/crates/v/netperf.svg)

A network (TCP-only) performance measurement tool written in Rust. Designed after iperf3's
original code.


This still work-in-progress but all basic features are implemented. Key differences from iperf3:
- Uses a different control protocol (not compatible with iperf3's servers or clients)
- Multi-threaded, parallel streams (-P) will be executed on different threads.
- Design simulates realworld server/client applications in terms of work scheduling.

![](https://github.com/AhmedSoliman/netperf/blob/master/assets/screenshot.png)

# Installation
```
cargo install netperf
```

# Usage
On one node you run netperf in server mode:
```
netperf -s
```
On a client node, you need to connect to that server (you will need an addressable IP address IPv6 is supported).
```
netperf -c ::1
```
By default, the test will use a single stream (client sends and server receives). You can control the number of parallel streams with `-P` and the direction of traffic with `-R/--bidir`

# Current Limitations
- Does not support configuring MSS, Congestion control algorithm.
- No UDP/STCP support.
- Does not collect extra stats like retransmits, cwnd, etc. (planned)


### License
Licensed under either of Apache License, Version 2.0 or MIT license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. 