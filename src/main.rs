use cling::prelude::*;
use netperf::common::opts::Opts;

#[tokio::main]
async fn main() -> ClingFinished<Opts> {
    Cling::parse_and_run().await
}
