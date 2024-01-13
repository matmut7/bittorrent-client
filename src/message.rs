use std::io::{self, Write};

#[derive(Debug)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request(u32, u32, u32),
    Piece(u32, u32, Vec<u8>),
    Cancel(u32, u32, u32),
}

impl Message {
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self {
            Message::KeepAlive => {
                buf.write_all(&[0, 0, 0, 0])?;
            }
            Message::Choke => {
                buf.write_all(&[0, 0, 0, 1, 0])?;
            }
            Message::Unchoke => {
                buf.write_all(&[0, 0, 0, 1, 1])?;
            }
            Message::Interested => {
                buf.write_all(&[0, 0, 0, 1, 2])?;
            }
            Message::NotInterested => {
                buf.write_all(&[0, 0, 0, 1, 3])?;
            }
            Message::Have(piece_index) => {
                buf.write_all(&[0, 0, 0, 5, 4])?;
                buf.write_all(&piece_index.to_be_bytes())?;
            }
            Message::Bitfield(bitfield) => {
                let length = 1 + bitfield.len() as u32;
                buf.write_all(&length.to_be_bytes())?;
                buf.write_all(&[5])?;
                buf.write_all(bitfield)?;
            }
            Message::Request(index, begin, length) => {
                buf.write_all(&[0, 0, 0, 13, 6])?;
                buf.write_all(&index.to_be_bytes())?;
                buf.write_all(&begin.to_be_bytes())?;
                buf.write_all(&length.to_be_bytes())?;
            }
            Message::Piece(index, begin, block) => {
                let length = 9 + block.len() as u32;
                buf.write_all(&length.to_be_bytes())?;
                buf.write_all(&[7])?;
                buf.write_all(&index.to_be_bytes())?;
                buf.write_all(&begin.to_be_bytes())?;
                buf.write_all(block)?;
            }
            Message::Cancel(index, begin, length) => {
                buf.write_all(&[0, 0, 0, 13, 8])?;
                buf.write_all(&index.to_be_bytes())?;
                buf.write_all(&begin.to_be_bytes())?;
                buf.write_all(&length.to_be_bytes())?;
            }
        }
        Ok(buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        match bytes {
            [0, 0, 0, 0] => Ok(Message::KeepAlive),
            [0, 0, 0, 1, 0] => Ok(Message::Choke),
            [0, 0, 0, 1, 1] => Ok(Message::Unchoke),
            [0, 0, 0, 1, 2] => Ok(Message::Interested),
            [0, 0, 0, 1, 3] => Ok(Message::NotInterested),
            [0, 0, 0, 5, 4, rest @ ..] if rest.len() == 4 => {
                let piece_index = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
                Ok(Message::Have(piece_index))
            }
            [_, _, _, _, 5, rest @ ..] => Ok(Message::Bitfield(rest.to_vec())),
            [_, _, _, _, 6, rest @ ..] if rest.len() == 12 => {
                let index = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
                let begin = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]);
                let length = u32::from_be_bytes([rest[8], rest[9], rest[10], rest[11]]);
                Ok(Message::Request(index, begin, length))
            }
            [_, _, _, _, 7, rest @ ..] if rest.len() >= 8 => {
                let index = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
                let begin = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]);
                let block = rest[8..].to_vec();
                Ok(Message::Piece(index, begin, block))
            }
            [_, _, _, _, 8, rest @ ..] if rest.len() == 12 => {
                let index = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
                let begin = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]);
                let length = u32::from_be_bytes([rest[8], rest[9], rest[10], rest[11]]);
                Ok(Message::Cancel(index, begin, length))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid message format",
            )),
        }
    }
}
