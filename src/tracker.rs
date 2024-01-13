use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::torrent_file::TorrentFile;
use crate::{CLIENT_ID, PORT};
use serde_bencode::de;
use serde_bytes::ByteBuf;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct TrackerResponse {
    peers: ByteBuf,
    interval: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl Default for Peer {
    fn default() -> Self {
        Peer {
            ip: Ipv4Addr::new(0, 0, 0, 0),
            port: Default::default(),
        }
    }
}

pub async fn fetch_peers(torrent: &TorrentFile) -> Vec<Peer> {
    // Fetch peers from tracker
    let mut params = HashMap::new();
    params.insert("peer_id", CLIENT_ID.to_string());
    params.insert("port", PORT.to_string());
    params.insert("uploaded", "0".to_string());
    params.insert("downloaded", "0".to_string());
    params.insert("compact", "1".to_string());
    params.insert("left", torrent.length.to_string());

    let http_client = reqwest::Client::new();

    // Fetch and decode
    let data = match http_client
        .get(format!(
            "{}?info_hash={}",
            torrent.announce, torrent.infohash_encoded
        ))
        .query(&params)
        .send()
        .await
    {
        Ok(response) => match response.bytes().await {
            Ok(body) => match de::from_bytes::<TrackerResponse>(&body) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("error decoding response:\n{}", e);
                    std::process::exit(1)
                }
            },
            Err(e) => {
                eprintln!("error while reading response:\n{}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("error while querying tracker:\n{}", e);
            std::process::exit(1);
        }
    };

    // Formatting information
    let peer_size = 6;
    if data.peers.len() % peer_size != 0 {
        eprintln!("received malformed peers from tracker");
        std::process::exit(1);
    }
    let num_peers = data.peers.len() / peer_size;
    let mut peers: Vec<Peer> = vec![Default::default(); num_peers];
    for (i, chunk) in data.peers.chunks(6).enumerate() {
        peers[i].ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        peers[i].port = u16::from_be_bytes([chunk[4], chunk[5]])
    }
    peers
}
