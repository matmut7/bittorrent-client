use std::{fs, io::Read};

use serde_bencode::de;
use serde_bytes::ByteBuf;

use crate::infohash::{infohash, url_encode};

// TODO: implement multi files mode

#[derive(Clone)]
pub struct TorrentFile {
    pub announce: String,
    pub name: String,
    pub piece_hashes: Vec<[u8; 20]>,
    pub piece_length: usize,
    pub length: usize,
    pub infohash: [u8; 20],
    pub infohash_encoded: String,
}

fn split_hashes(pieces: &ByteBuf) -> Vec<[u8; 20]> {
    let hash_len = 20;
    if pieces.len() % hash_len != 0 {
        eprintln!("received malformed pieces of length {}", pieces.len());
        std::process::exit(1);
    }
    let num_hashes = pieces.len() / hash_len;
    let mut hashes: Vec<[u8; 20]> = vec![[0u8; 20]; num_hashes];
    for (i, chunk) in pieces.chunks_exact(hash_len).enumerate() {
        hashes[i].copy_from_slice(chunk)
    }
    hashes
}

impl TorrentFile {
    pub fn from_bencode(torrent: &BencodeTorrent, info: &BencodeInfo) -> Self {
        let infohash = infohash(info);
        TorrentFile {
            announce: torrent
                .announce
                .clone()
                .expect("missing field in torrent file: announce"),
            name: info.name.clone(),
            piece_hashes: split_hashes(&info.pieces),
            piece_length: usize::try_from(info.piece_length).expect("piece_length is too big"),
            length: usize::try_from(info.length.expect(
                "missing field in torrent file: length (multi file mode is not yet implemented)",
            ))
            .expect("length is too big"),
            infohash,
            infohash_encoded: url_encode(&infohash),
        }
    }

    pub fn calculate_piece_size(&self, index: usize) -> usize {
        let (start, end) = self.calculate_bound_for_piece(index);
        end - start
    }

    pub fn calculate_bound_for_piece(&self, index: usize) -> (usize, usize) {
        let start = index * self.piece_length;
        let end = std::cmp::min(start + self.piece_length, self.length);
        (start, end)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct BencodeInfo {
    pub name: String,
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: i64,
    #[serde(default)]
    pub length: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct BencodeTorrent {
    pub info: BencodeInfo,
    #[serde(default)]
    pub announce: Option<String>,
}

pub fn read_and_decode(file_name: &String) -> TorrentFile {
    // Open file
    let mut file = match fs::File::open(file_name) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("error opening file:\n{}", e);
            std::process::exit(1);
        }
    };

    // Read file
    let mut bytes = Vec::new();
    match file.read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("error reading file:\n{}", e);
            std::process::exit(1);
        }
    }

    match de::from_bytes::<BencodeTorrent>(&bytes) {
        Ok(torrent) => TorrentFile::from_bencode(&torrent, &torrent.info),
        Err(e) => {
            eprintln!("error decoding torrent file:\n{}", e);
            std::process::exit(1);
        }
    }
}
