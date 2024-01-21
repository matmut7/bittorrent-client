use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc::Sender, Mutex},
    time,
};

use crate::{
    bitfield::bitfield_has_piece,
    controller::{PieceResult, PieceWork, WorkerStatusMessage},
    message::Message,
    peer::{handshake, init_connection},
    torrent_file::TorrentFile,
    tracker::Peer,
};

pub struct State {
    pub piece_progress: PieceProgress,
    pub bitfield: Vec<u8>,
    pub peer_choking: bool,
}

#[derive(Default)]
pub struct PieceProgress {
    pub buf: Vec<u8>,
    pub num_downloaded_bytes: usize,
    num_requested_bytes: usize,
    backlog_length: usize,
}

const MAX_BLOCK_SIZE: usize = 16384;
const MAX_BACKLOG: usize = 5;

pub async fn read_message(tcp_stream: &mut TcpStream) -> Result<Message> {
    let mut len_buf = [0u8; 4];
    tcp_stream.read_exact(&mut len_buf).await?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 {
        return Ok(Message::KeepAlive);
    }
    let mut payload_buf = vec![0u8; len];
    tcp_stream.read_exact(&mut payload_buf).await?;

    let mut whole_msg_bytes: Vec<u8> = Vec::with_capacity(len_buf.len() + payload_buf.len());
    whole_msg_bytes.extend_from_slice(&len_buf);
    whole_msg_bytes.extend_from_slice(&payload_buf);
    Message::from_bytes(&whole_msg_bytes)
}

pub async fn write_message(tcp_stream: &mut TcpStream, message: &Message) -> Result<()> {
    let bytes = message.to_bytes()?;
    tcp_stream.write_all(&bytes).await?;
    Ok(())
}

pub const TIMEOUT: u64 = 10;

pub async fn start_download_worker(
    peer: &Peer,
    torrent_file: &TorrentFile,
    work_queue: &Arc<Mutex<VecDeque<PieceWork>>>,
    result_sender: &Sender<PieceResult>,
    status_sender: &Sender<WorkerStatusMessage>,
) -> Result<()> {
    // Open connection and handshake with peer
    let mut tcp_stream =
        match time::timeout(Duration::new(TIMEOUT, 0), handshake(peer, torrent_file)).await {
            Ok(Ok(tcp_stream)) => tcp_stream,
            Ok(Err(_)) => {
                // eprintln!("could not open connection with peer {}", peer.ip);
                // dbg!(e);
                return Err(anyhow!("handshake"));
            }
            Err(_) => {
                // eprintln!("timed out opening connection with peer {}", peer.ip);
                return Err(anyhow!("handshake"));
            }
        };

    let mut state =
        match time::timeout(Duration::new(TIMEOUT, 0), init_connection(&mut tcp_stream)).await {
            Ok(Ok(state)) => state,
            _ => {
                // eprint!("error reading bitfield from peer");
                return Err(anyhow!("init"));
            }
        };

    while let Some(piece_work) = {
        let mut guard = work_queue.lock().await;
        guard.pop_front()
    } {
        // Check if peer has piece
        if !bitfield_has_piece(&state.bitfield, piece_work.index) {
            work_queue.lock().await.push_back(piece_work);
            continue;
        }

        state.piece_progress = PieceProgress::default();
        state.piece_progress.buf.resize(piece_work.length, 0u8);

        status_sender
            .send(WorkerStatusMessage {
                connected: true,
                id: peer.ip,
            })
            .await?;

        while state.piece_progress.num_downloaded_bytes < piece_work.length {
            // Send Request messages until backlog is full
            while state.piece_progress.backlog_length < MAX_BACKLOG
                && state.piece_progress.num_requested_bytes < piece_work.length
            {
                let block_size = std::cmp::min(
                    MAX_BLOCK_SIZE,
                    piece_work.length - state.piece_progress.num_requested_bytes,
                );
                match time::timeout(
                    Duration::new(TIMEOUT, 0),
                    write_message(
                        &mut tcp_stream,
                        &Message::Request(
                            u32::try_from(piece_work.index).expect("pieces are to big"),
                            u32::try_from(state.piece_progress.num_requested_bytes)
                                .expect("pieces are too big"),
                            u32::try_from(block_size).expect("pieces are too big"),
                        ),
                    ),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        state.piece_progress.backlog_length += 1;
                        state.piece_progress.num_requested_bytes += block_size;
                    }
                    _ => {
                        work_queue.lock().await.push_back(piece_work);
                        return Err(anyhow!("request"));
                    }
                }
            }

            let message =
                match time::timeout(Duration::new(10, 0), read_message(&mut tcp_stream)).await {
                    Ok(Ok(message)) => message,
                    e => {
                        work_queue.lock().await.push_back(piece_work);
                        return Err(anyhow!("reading message {:?}", e));
                    }
                };

            match message {
                Message::Piece(received_piece_index, received_block_index, payload) => {
                    if let Err(e) = validate_piece_message(
                        &piece_work,
                        &state,
                        received_piece_index,
                        received_block_index,
                        &payload,
                    ) {
                        work_queue.lock().await.push_back(piece_work);
                        return Err(e);
                    }

                    // FIX: remove all as
                    state.piece_progress.buf[received_block_index as usize
                        ..received_block_index as usize + payload.len()]
                        .copy_from_slice(&payload);
                    state.piece_progress.num_downloaded_bytes += payload.len();
                    state.piece_progress.backlog_length -= 1;
                }
                Message::Choke => {
                    state.peer_choking = true;
                }
                Message::Unchoke => {
                    state.peer_choking = false;
                }
                Message::KeepAlive => {}

                // other cases
                message => {
                    work_queue.lock().await.push_back(piece_work);
                    return Err(anyhow!("unsupported behaviour from peer {:?}", message));
                }
            }
        }

        // Piece is complete
        if (end_download(&piece_work, &state, &mut tcp_stream, result_sender).await).is_err() {
            work_queue.lock().await.push_back(piece_work);
            return Err(anyhow!("ending download"));
        }
    }
    Ok(())
}

fn check_integrity(piece_work: &PieceWork, state: &State) -> bool {
    let hash = <Sha1 as Digest>::digest(&state.piece_progress.buf);
    hash.as_slice() == piece_work.hash
}

fn validate_piece_message(
    piece_work: &PieceWork,
    state: &State,
    received_piece_index: u32,
    received_block_index: u32,
    payload: &Vec<u8>,
) -> Result<()> {
    if payload.len() < 8 {
        return Err(anyhow!("payload too short"));
    }
    if received_piece_index != u32::try_from(piece_work.index).expect("pieces are too big") {
        return Err(anyhow!("got wrong piece from peer"));
    }
    if received_block_index as usize > state.piece_progress.buf.len() {
        return Err(anyhow!("piece offset too high"));
    }
    if received_block_index as usize + payload.len() > state.piece_progress.buf.len() {
        return Err(anyhow!("data too long for offset"));
    }
    Ok(())
}

async fn end_download(
    piece_work: &PieceWork,
    state: &State,
    tcp_stream: &mut TcpStream,
    result_sender: &Sender<PieceResult>,
) -> Result<()> {
    if !check_integrity(piece_work, state) {
        return Err(anyhow!("wrong hash for piece {:?}", piece_work));
    }

    time::timeout(
        Duration::new(TIMEOUT, 0),
        write_message(tcp_stream, &Message::Have(piece_work.index as u32)),
    )
    .await??;

    result_sender
        .send(PieceResult {
            index: piece_work.index,
            buf: state.piece_progress.buf.clone(),
        })
        .await?;

    Ok(())
}
