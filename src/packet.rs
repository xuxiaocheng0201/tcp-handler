use std::io::Error;
use bytes::BytesMut;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use crate::config::get_max_packet_size;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Packet size {0} is larger than the maximum allowed packet size {1}.")]
    TooLarge(usize, usize),
    #[error("During io packet.")]
    IO(#[from] Error),
}

#[inline]
fn check_bytes_len(len: usize) -> Result<(), PacketError> {
    let config = get_max_packet_size();
    if len > config { Err(PacketError::TooLarge(len, config)) } else { Ok(()) }
}

pub async fn write_packet<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, bytes: &[u8]) -> Result<(), PacketError> {
    check_bytes_len(bytes.len())?;
    stream.write_u128_varint(bytes.len() as u128).await?;
    stream.write_more(&bytes).await?;
    #[cfg(feature = "auto_flush")]
    stream.flush().await?;
    Ok(())
}

pub async fn read_packet<R: AsyncReadExt + Unpin + Send>(stream: &mut R) -> Result<BytesMut, PacketError> {
    let len = stream.read_u128_varint().await? as usize;
    check_bytes_len(len)?;
    let mut buf = BytesMut::zeroed(len);
    stream.read_more(&mut buf).await?;
    Ok(buf)
}
