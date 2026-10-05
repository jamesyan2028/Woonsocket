use std::io::{self, Read, Write};

pub fn encode<T: serde::Serialize>(buf: &mut Vec<u8>, msg: &T) -> io::Result<()> {
    let len_pos = buf.len();
    buf.extend_from_slice(&[0u8; 4]); // placeholder for the length
    bincode::serialize_into(&mut *buf, msg).map_err(to_io)?;
    let len = (buf.len() - len_pos - 4) as u32;
    buf[len_pos..len_pos + 4].copy_from_slice(&len.to_be_bytes());
    Ok(())
}

pub fn send<W: Write, T: serde::Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let mut buf = Vec::with_capacity(64);
    encode(&mut buf, msg)?;
    w.write_all(&buf)
}

pub fn receive<R: Read, T: serde::de::DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let mut buf = vec![0u8; u32::from_be_bytes(len) as usize];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(to_io)
}

fn to_io(e: bincode::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}
