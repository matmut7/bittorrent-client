use crate::{
    message::Message,
    torrent_file::TorrentFile,
    tracker::Peer,
    worker::{read_message, write_message, PieceProgress, State},
    CLIENT_ID,
};
use std::error::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const PSTR: &[u8] = b"BitTorrent protocol";

pub async fn handshake(peer: &Peer, torrent: &TorrentFile) -> Result<TcpStream, Box<dyn Error>> {
    // Open TCP stream
    let mut stream = TcpStream::connect((peer.ip, peer.port)).await?;

    let pstr_len = PSTR.len() as u8;
    let reserved = [0u8; 8];

    let mut handshake = [0u8; 49 + PSTR.len()];
    handshake[0] = pstr_len;
    handshake[1..20].copy_from_slice(PSTR);
    handshake[20..28].copy_from_slice(&reserved);
    handshake[28..48].copy_from_slice(&torrent.infohash);
    handshake[48..].copy_from_slice(CLIENT_ID.as_bytes());

    stream.write_all(&handshake).await?;

    let mut response = [0u8; 49 + PSTR.len()];
    stream.read_exact(&mut response).await?;

    if response[28..48] != torrent.infohash {
        return Err("wrong infohash from peer".into());
    }

    Ok(stream)
}

pub async fn init_connection(tcp_stream: &mut TcpStream) -> Result<State, Box<dyn Error>> {
    // Wait for a bitfield as first message
    let bitfield = match read_message(tcp_stream).await? {
        Message::Bitfield(payload) => payload,
        message => {
            return Err(format!("expected bitfield but got {:?}", message).into());
        }
    };

    // Notify we're interested
    write_message(tcp_stream, &Message::Interested).await?;

    // Waiting to be unchoked
    read_message(tcp_stream).await?;

    Ok(State {
        piece_progress: PieceProgress::default(),
        bitfield,
        peer_choking: true,
        _am_choking: true,
        _am_interested: true,
        _peer_interested: false,
    })
}
