use std::io::Error;
#[cfg(feature = "encrypt")]
use aes_gcm::{
    AeadCore, AeadInPlace, Aes256Gcm, Nonce,
    aead::Aead,
    aead::rand_core::OsRng as AesRng,
    aead::Error as AesGcmError,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use variable_len_reader::VariableWritable;
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

pub(crate) async fn write_packet<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, bytes: &Bytes) -> Result<(), PacketError> {
    let mut bytes = bytes.clone();
    check_bytes_len(bytes.remaining())?;
    stream.write_u128_varint(bytes.remaining() as u128).await?;
    while bytes.has_remaining() {
        let len = stream.write_more(bytes.chunk()).await?;
        bytes.advance(len);
    }
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

pub async fn send<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes) -> Result<(), PacketError> {
    write_packet(stream, message).await
}
pub async fn recv<R: AsyncReadExt + Unpin + Send>(stream: &mut R) -> Result<BytesMut, PacketError> {
    read_packet(stream).await
}

#[cfg(feature = "encrypt")]
#[deprecated(since = "0.1.0", note = "Please use `send_with_dynamic_encrypt` instead.")]
pub async fn send_with_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes, cipher: &AesCipher) -> Result<(), PacketError> {
    let (cipher, nonce) = cipher;
    let message = cipher.encrypt(nonce, &*message.to_vec())?;
    write_packet(stream, &Bytes::from(message)).await
}
#[cfg(feature = "encrypt")]
#[deprecated(since = "0.1.0", note = "Please use `recv_with_dynamic_encrypt` instead.")]
pub async fn recv_with_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: &AesCipher) -> Result<BytesMut, PacketError> {
    let (cipher, nonce) = cipher;
    let bytes = read_packet(stream).await?;
    let mut message = BytesMut::new();
    cipher.decrypt_in_place(nonce, &*bytes, &mut message)?;
    Ok(message)
}

#[cfg(feature = "encrypt")]
pub async fn client_send_with_dynamic_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes, cipher: AesCipher) -> Result<AesCipher, PacketError> {
    let (cipher, nonce) = cipher;
    let mut message = BytesMut::from(message.as_ref());
    cipher.encrypt_in_place(&nonce, &[], &mut message)?;
    write_packet(stream, &message.into()).await?;
    Ok((cipher, nonce))
}
#[cfg(feature = "encrypt")]
pub async fn server_recv_with_dynamic_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: AesCipher) -> Result<(BytesMut, AesCipher), PacketError> {
    let (cipher, nonce) = cipher;
    let mut message = read_packet(stream).await?;
    cipher.decrypt_in_place(&nonce, &[], &mut message)?;
    Ok((message, (cipher, nonce)))
}
#[cfg(feature = "encrypt")]
pub async fn server_send_with_dynamic_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes, cipher: AesCipher) -> Result<AesCipher, PacketError> {
    let (cipher, nonce) = cipher;
    let new_nonce = Aes256Gcm::generate_nonce(&mut AesRng);
    debug_assert_eq!(12, nonce.len());
    let mut message = BytesMut::from(message.as_ref());
    cipher.encrypt_in_place(&nonce, &[], &mut message)?;
    write_packet(stream, &message.into()).await?;
    stream.write_more(&new_nonce).await?;
    Ok((cipher, new_nonce))
}
#[cfg(feature = "encrypt")]
pub async fn client_recv_with_dynamic_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: AesCipher) -> Result<(BytesMut, AesCipher), PacketError> {
    let (cipher, nonce) = cipher;
    let mut message = read_packet(stream).await?;
    cipher.decrypt_in_place(&nonce, &[], &mut message)?;
    let mut new_nonce = [0; 12];
    stream.read_more(&mut new_nonce).await?;
    Ok((message, (cipher, Nonce::from(new_nonce))))
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use bytes::{Buf, BufMut, BytesMut};
    use variable_len_reader::{VariableReadable, VariableWritable};
    use crate::packet::{client_recv_with_dynamic_encrypt, client_send_with_dynamic_encrypt, recv, send, server_recv_with_dynamic_encrypt, server_send_with_dynamic_encrypt};
    use crate::starter::{client_init, client_init_with_encrypt, client_start, client_start_with_encrypt, server_init, server_init_with_encrypt, server_start, server_start_with_encrypt};
    use crate::starter::test::create;

    #[tokio::test]
    async fn raw() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init(&mut client, &"a", &"1").await;
        let s = server_init(&mut server, &"a", |v| v == "1").await;
        server_start(&mut server, s).await?;
        client_start(&mut client, c).await?;

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        send(&mut client, &writer.into_inner().into()).await?;

        let mut reader = recv(&mut server).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello server.", message);
        Ok(())
    }

    #[tokio::test]
    async fn encrypt() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init_with_encrypt(&mut client, &"a", &"1").await;
        let s = server_init_with_encrypt(&mut server, &"a", |v| v == "1").await;
        let server_cipher = server_start_with_encrypt(&mut server, s).await?;
        let client_cipher = client_start_with_encrypt(&mut client, c).await?;

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        let client_cipher = client_send_with_dynamic_encrypt(&mut client, &writer.into_inner().into(), client_cipher).await?;

        let (reader, server_cipher) = server_recv_with_dynamic_encrypt(&mut server, server_cipher).await?;
        let message = reader.reader().read_string()?;
        assert_eq!("hello server.", message);

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello client.")?;
        let _server_cipher = server_send_with_dynamic_encrypt(&mut client, &writer.into_inner().into(), server_cipher).await?;

        let (reader, _client_cipher) = client_recv_with_dynamic_encrypt(&mut server, client_cipher).await?;
        let message = reader.reader().read_string()?;
        assert_eq!("hello client.", message);

        Ok(())
    }
}
