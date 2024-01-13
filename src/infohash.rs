use crate::torrent_file::BencodeInfo;
use serde_bencode::ser;
use sha1::{Digest, Sha1};

static ALLOWED_CHARS: &[u8] = &[
    b'.', b'-', b'_', b'~', b'-', b'_', b'.', b'!', b'~', b'*', b'\'', b'(', b')', b';', b'/',
    b'?', b':', b'@', b'&', b'=', b'+', b'$', b',', b'#',
];

fn is_allowed_byte(byte: &u8) -> bool {
    // checks if the byte is within the ranges of '0-9', 'a-z', 'A-Z', or is '.', '-', '_', '~'
    byte.is_ascii_digit()
        || byte.is_ascii_lowercase()
        || byte.is_ascii_uppercase()
        || ALLOWED_CHARS.contains(byte)
}

pub fn infohash(info: &BencodeInfo) -> [u8; 20] {
    let encoded = match ser::to_bytes::<BencodeInfo>(info) {
        Ok(encoded) => encoded,
        Err(e) => {
            eprintln!("error while encoding torrent info:\n{}", e);
            std::process::exit(1);
        }
    };

    <Sha1 as Digest>::digest(encoded).into()
}

pub fn url_encode(bytes: &[u8]) -> String {
    let mut result = Vec::new();
    for byte in bytes {
        if is_allowed_byte(byte) {
            result.push(std::str::from_utf8(&[*byte]).unwrap().to_string())
        } else {
            result.push(format!("%{:02x}", byte))
        }
    }
    result.join("")
}
