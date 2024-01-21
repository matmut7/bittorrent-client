# Bittorrent client

Educational implementation of a simple Bittorrent client, following this great
guide: [https://blog.jse.li/posts/torrent/](https://blog.jse.li/posts/torrent/).
It is capable of downloading a single file torrent.

Multiple aspects of the protocol are missing:
[multi file torrents](https://wiki.theory.org/BitTorrentSpecification#Info_in_Multiple_File_Mode),
[trackers announce-list](https://wiki.theory.org/BitTorrentSpecification#Metainfo_File_Structure),
seeding, etc.

What I worked with:

- HTTP with [reqwest](https://crates.io/crates/reqwest)
- TCP streams
- byte manipulation
- multithreading with [tokio](https://crates.io/crates/tokio)
  - shared memory with Arc<Mutex<>>
  - MPSC channels
- [serde](https://crates.io/crates/serde) and
  [serde_bencode](https://crates.io/crates/serde_bencode)

## Usage

To see it in action:

```shell
cargo run assets/debian.torrent
```
