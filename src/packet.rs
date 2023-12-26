use std::io::Error;
#[cfg(feature = "encrypt")]
use aes_gcm::{
    AeadCore, Aes256Gcm, Nonce,
    aead::Aead,
    aead::rand_core::OsRng as AesRng,
    aead::Error as AesGcmError,
};
use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use crate::config::get_max_packet_size;
#[cfg(feature = "encrypt")]
use crate::starter::AesCipher;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Packet size {0} is larger than the maximum allowed packet size {1}.")]
    TooLarge(usize, usize),
    #[error("During io packet.")]
    IO(#[from] Error),
    #[cfg(feature = "encrypt")]
    #[error("During encrypting/decrypting bytes.")]
    AES(#[from] AesGcmError)
}

#[inline]
fn check_bytes_len(len: usize) -> Result<(), PacketError> {
    let config = get_max_packet_size();
    if len > config { Err(PacketError::TooLarge(len, config)) } else { Ok(()) }
}

pub(crate) async fn write_packet<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, bytes: &[u8]) -> Result<(), PacketError> {
    check_bytes_len(bytes.len())?;
    stream.write_u128_varint(bytes.len() as u128).await?;
    stream.write_more(&bytes).await?;
    #[cfg(feature = "auto_flush")]
    stream.flush().await?;
    Ok(())
}

pub(crate) async fn read_packet<R: AsyncReadExt + Unpin + Send>(stream: &mut R) -> Result<BytesMut, PacketError> {
    let len = stream.read_u128_varint().await? as usize;
    check_bytes_len(len)?;
    let mut buf = BytesMut::zeroed(len);
    stream.read_more(&mut buf).await?;
    Ok(buf)
}

pub async fn send<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, bytes: &[u8]) -> Result<(), PacketError> {
    write_packet(stream, bytes).await
}

pub async fn recv<R: AsyncReadExt + Unpin + Send>(stream: &mut R) -> Result<Bytes, PacketError> {
    read_packet(stream).await.map(|m| m.into())
}

#[cfg(feature = "encrypt")]
#[deprecated(since = "0.1.0", note = "Please use `send_with_dynamic_encrypt` instead.")]
pub async fn send_with_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &[u8], cipher: &AesCipher) -> Result<(), PacketError> {
    let (cipher, nonce) = cipher;
    let message = cipher.encrypt(nonce, message)?;
    write_packet(stream, &message).await
}

#[cfg(feature = "encrypt")]
#[deprecated(since = "0.1.0", note = "Please use `recv_with_dynamic_encrypt` instead.")]
pub async fn recv_with_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: &AesCipher) -> Result<Bytes, PacketError> {
    let (cipher, nonce) = cipher;
    let message = read_packet(stream).await?;
    let message = cipher.decrypt(nonce, &*message)?;
    Ok(Bytes::from(message))
}

#[cfg(feature = "encrypt")]
pub async fn client_send_with_dynamic_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &[u8], cipher: AesCipher) -> Result<AesCipher, PacketError> {
    let (cipher, nonce) = cipher;
    let message = cipher.encrypt(&nonce, message)?;
    write_packet(stream, &message).await?;
    Ok((cipher, nonce))
}

#[cfg(feature = "encrypt")]
pub async fn server_recv_with_dynamic_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: AesCipher) -> Result<(Bytes, AesCipher), PacketError> {
    let (cipher, nonce) = cipher;
    let message = read_packet(stream).await?;
    let message = cipher.decrypt(&nonce, &*message)?;
    Ok((Bytes::from(message), (cipher, nonce)))
}

#[cfg(feature = "encrypt")]
pub async fn server_send_with_dynamic_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &[u8], cipher: AesCipher) -> Result<AesCipher, PacketError> {
    let (cipher, nonce) = cipher;
    let new_nonce = Aes256Gcm::generate_nonce(&mut AesRng);
    assert_eq!(12, nonce.len());
    let message = cipher.encrypt(&nonce, message)?;
    write_packet(stream, &message).await?;
    stream.write_more(&new_nonce).await?;
    Ok((cipher, new_nonce))
}

#[cfg(feature = "encrypt")]
pub async fn client_recv_with_dynamic_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: AesCipher) -> Result<(Bytes, AesCipher), PacketError> {
    let (cipher, nonce) = cipher;
    let message = read_packet(stream).await?;
    let message = cipher.decrypt(&nonce, &*message)?;
    let mut new_nonce = [0; 12];
    stream.read_more(&mut new_nonce).await?;
    Ok((Bytes::from(message), (cipher, Nonce::from(new_nonce))))
}
