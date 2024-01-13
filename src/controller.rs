use std::{
    collections::{HashMap, VecDeque},
    env,
    fs::File,
    io::Write,
    net::Ipv4Addr,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::{mpsc, Mutex},
    time,
};

use crate::{torrent_file::TorrentFile, tracker::fetch_peers, worker::start_download_worker};

#[derive(Debug)]
pub struct PieceWork {
    pub index: usize,
    pub hash: [u8; 20],
    pub length: usize,
}

#[derive(Debug)]
pub struct PieceResult {
    pub index: usize,
    pub buf: Vec<u8>,
}

pub struct WorkerStatusMessage {
    pub connected: bool,
    pub id: Ipv4Addr,
}

pub async fn download_file(torrent_file: &TorrentFile) {
    // Fetch peers list from tracker
    let peers = fetch_peers(torrent_file).await;

    let work_queue = Arc::new(Mutex::new(VecDeque::new()));
    let (result_sender, mut result_receiver) = mpsc::channel::<PieceResult>(100);
    let (status_sender, mut status_receiver) = mpsc::channel::<WorkerStatusMessage>(100);

    // Init work queue
    {
        let mut queue = work_queue.lock().await;
        for (index, hash) in torrent_file.piece_hashes.iter().enumerate() {
            let length = torrent_file.calculate_piece_size(index);
            let piece_work = PieceWork {
                index,
                hash: hash.to_owned(),
                length,
            };
            queue.push_back(piece_work)
        }
    }

    // Start logger thread
    tokio::spawn(async move {
        let mut connected_workers = 0;
        let mut workers_status: HashMap<Ipv4Addr, bool> = HashMap::new();
        while let Some(status) = status_receiver.recv().await {
            if status.connected && !workers_status.get(&status.id).unwrap_or(&false) {
                connected_workers += 1;
                workers_status.insert(status.id, true);
                println!(
                    "connected workers: {}, new connection: {}",
                    connected_workers, &status.id
                );
            } else if !status.connected && *workers_status.get(&status.id).unwrap_or(&false) {
                connected_workers -= 1;
                workers_status.insert(status.id, false);
                println!(
                    "connected workers: {}, connection stopped: {}",
                    connected_workers, &status.id
                );
            }
        }
    });

    for peer in peers {
        let thread_result_sender = result_sender.clone();
        let thread_status_sender = status_sender.clone();
        let thread_torrent_file = torrent_file.clone(); // NOTE: use Arc to avoid this cloning ?
        let thread_work_queue = work_queue.clone();
        tokio::spawn(async move {
            while (start_download_worker(
                &peer,
                &thread_torrent_file,
                &thread_work_queue,
                &thread_result_sender,
                &thread_status_sender,
            )
            .await)
                .is_err()
            {
                if let Err(e) = thread_status_sender
                    .send(WorkerStatusMessage {
                        connected: false,
                        id: peer.ip,
                    })
                    .await
                {
                    eprint!("error sending status to main thread:\n{}", e);
                }
                time::sleep(Duration::from_secs(3)).await;
            }
        });
    }

    // Collect results pieces
    let mut buf = vec![0u8; torrent_file.length];
    let mut done_pieces = 0;

    // Bandwidth display
    let mut start_time = Instant::now();
    let mut window_bytes_received = 0;
    let window_duration = Duration::from_secs(3);

    while done_pieces < torrent_file.piece_hashes.len() {
        let result_piece = result_receiver
            .recv()
            .await
            .expect("result channel closed unexpectedly");
        let (start, end) = torrent_file.calculate_bound_for_piece(result_piece.index);
        buf[start..end].copy_from_slice(&result_piece.buf);
        done_pieces += 1;
        window_bytes_received += result_piece.buf.len();

        if start_time.elapsed() >= window_duration {
            let bandwidth = window_bytes_received as u64 / window_duration.as_secs() / 1000;
            println!(
                "{} Ko/s, {}/{} pieces",
                bandwidth,
                done_pieces,
                torrent_file.piece_hashes.len()
            );
            window_bytes_received = 0;
            start_time = Instant::now();
        }
    }

    let target_dir = env::current_dir().unwrap_or(PathBuf::from_str("/tmp/").unwrap());
    let file_path = target_dir.join(&torrent_file.name);
    let mut file = File::create(&file_path).expect("failed to create file");
    file.write_all(&buf).expect("failed to write data to file");
    println!("file downloaded successfully to {:?}", file_path);
}
