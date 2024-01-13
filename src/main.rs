pub mod bitfield;
pub mod controller;
pub mod infohash;
pub mod message;
pub mod peer;
pub mod torrent_file;
pub mod tracker;
pub mod worker;

use crate::controller::download_file;
use crate::torrent_file::read_and_decode;
use std::env;

#[macro_use]
extern crate serde_derive;

static PORT: u16 = 6881;
static CLIENT_ID: &str = "d198c9596d8ccf89a0e5";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Get torrent file name as parameter
    if args.len() != 2 {
        eprintln!("provide one only argument, a torrent file's path");
        std::process::exit(1);
    }
    let torrent_file_name = &args[1];

    // Read and decode torrent file
    let torrent_file = read_and_decode(torrent_file_name);

    download_file(&torrent_file).await;
}
