use std::io{Read, Write};

pub fn send<W: Write, T: serde::Serialize>(w: &mut W, msg: &T) -> std:io::Result<()> {
    let bytes = bincode::serailize(msg).unwrap();
    w.write_all(&(bin.len() as u32).to_be_bytes())?;
    w.write_all(&bytes)
}

pub fn receive<R: Read, T: serde::de::DeserializeOwned>(r: &mut R) -> std::io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let mut buf = vec![0u8; u32::from_be_bytes(len) as usize];
    r.read_exact(&mut buf)?;
    Ok(bincode::deserialize(&buf).unwrap())
}