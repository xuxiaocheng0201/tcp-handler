//! Compression and encryption protocol.
//!
//! Recommended to use this protocol.
//!
//! # Example
//! ```rust
//! use anyhow::Result;
//! use bytes::{Buf, BufMut, BytesMut};
//! use flate2::Compression;
//! use tcp_handler::compress_encrypt::*;
//! use tokio::net::{TcpListener, TcpStream};
//! use variable_len_reader::{VariableReadable, VariableWritable};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let server = TcpListener::bind("localhost:0").await?;
//!     let mut client = TcpStream::connect(server.local_addr()?).await?;
//!     let (mut server, _) = server.accept().await?;
//!     let level = Compression::default();
//!
//!     let c_init = client_init(&mut client, &"test", &"0").await;
//!     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
//!     let mut s_cipher = server_start(&mut server, s_init).await?;
//!     let mut c_cipher = client_start(&mut client, c_init).await?;
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello server.")?;
//!     let bytes = writer.into_inner().into();
//!     c_cipher = send(&mut client, &bytes, c_cipher, level).await?;
//!
//!     let (reader, s) = recv(&mut server, s_cipher).await?;
//!     let mut reader = reader.reader(); s_cipher = s;
//!     let message = reader.read_string()?;
//!     assert_eq!("hello server.", message);
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello client.")?;
//!     let bytes = writer.into_inner().into();
//!     s_cipher = send(&mut server, &bytes, s_cipher, level).await?;
//!
//!     let (reader, c) = recv(&mut client, c_cipher).await?;
//!     let mut reader = reader.reader(); c_cipher = c;
//!     let message = reader.read_string()?;
//!     assert_eq!("hello client.", message);
//!
//!     # let _ = s_cipher;
//!     # let _ = c_cipher;
//!     Ok(())
//! }
//! ```

use aead::{AeadCore, AeadInPlace};
use aes_gcm::aead::rand_core::OsRng as AesRng;
use aes_gcm::{Aes256Gcm, Nonce};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::Compression;
use flate2::write::{DeflateDecoder, DeflateEncoder};
use rsa::rand_core::OsRng as RsaRng;
use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::traits::PublicKeyParts;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::{VariableReadable, VariableWritable};
use crate::common::{AesCipher, PacketError, read_head, read_packet, StarterError, write_head, write_packet};

pub async fn client_init<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<RsaPrivateKey, StarterError> {
    let key = RsaPrivateKey::new(&mut RsaRng, 2048)?;
    let mut writer = write_head(stream, identifier, version, true, true).await?;
    writer.write_u8_vec(&key.n().to_bytes_le())?;
    writer.write_u8_vec(&key.e().to_bytes_le())?;
    write_packet(stream, &writer.into_inner().into()).await?;
    Ok(key)
}

pub async fn server_init<R: AsyncReadExt + Unpin + Send, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<RsaPublicKey, StarterError> {
    let mut reader = read_head(stream, identifier, version, true, true).await?;
    let n = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let e = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let key = RsaPublicKey::new(n, e)?;
    Ok(key)
}

#[inline]
pub async fn server_start<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, last: Result<RsaPublicKey, StarterError>) -> Result<AesCipher, StarterError> {
    crate::encrypt::server_start(stream, last).await
}

#[inline]
pub async fn client_start<R: AsyncReadExt + Unpin + Send>(stream: &mut R, last: Result<RsaPrivateKey, StarterError>) -> Result<AesCipher, StarterError> {
    crate::encrypt::client_start(stream, last).await
}

pub async fn send<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes, cipher: AesCipher, level: Compression) -> Result<AesCipher, PacketError> {
    let (cipher, nonce) = cipher;
    let new_nonce = Aes256Gcm::generate_nonce(&mut AesRng);
    debug_assert_eq!(12, nonce.len());
    let mut message = message.clone();
    let mut encoder = DeflateEncoder::new(BytesMut::new().writer(), level);
    encoder.write_more(&new_nonce)?;
    while message.has_remaining() {
        let len = encoder.write_more(message.chunk())?;
        message.advance(len);
    }
    let mut bytes = encoder.finish()?.into_inner();
    cipher.encrypt_in_place(&nonce, &[], &mut bytes)?;
    write_packet(stream, &bytes.into()).await?;
    Ok((cipher, new_nonce))
}

pub async fn recv<R: AsyncReadExt + Unpin + Send>(stream: &mut R, cipher: AesCipher) -> Result<(BytesMut, AesCipher), PacketError> {
    let (cipher, nonce) = cipher;
    let mut bytes = read_packet(stream).await?;
    cipher.decrypt_in_place(&nonce, &[], &mut bytes)?;
    let mut decoder = DeflateDecoder::new(BytesMut::new().writer());
    while bytes.has_remaining() {
        let len = decoder.write_more(bytes.chunk())?;
        bytes.advance(len);
    }
    let mut reader = decoder.finish()?.into_inner().reader();
    let mut new_nonce = [0; 12];
    reader.read_more(&mut new_nonce)?;
    Ok((reader.into_inner(), (cipher, Nonce::from(new_nonce))))
}


#[cfg(test)]
mod test {
    use anyhow::Result;
    use bytes::{Buf, BufMut, BytesMut};
    use flate2::Compression;
    use variable_len_reader::{VariableReadable, VariableWritable};
    use crate::compress_encrypt::{recv, send};
    use crate::common::test::create;

    #[tokio::test]
    async fn connect() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = crate::compress_encrypt::client_init(&mut client, &"a", &"1").await;
        let s = crate::compress_encrypt::server_init(&mut server, &"a", |v| v == "1").await;
        let mut s_cipher = crate::compress_encrypt::server_start(&mut server, s).await?;
        let mut c_cipher = crate::compress_encrypt::client_start(&mut client, c).await?;

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        let bytes = writer.into_inner().into();
        c_cipher = send(&mut client, &bytes, c_cipher, Compression::default()).await?;

        let (reader, s) = recv(&mut server, s_cipher).await?;
        let mut reader = reader.reader(); s_cipher = s;
        let message = reader.read_string()?;
        assert_eq!("hello server.", message);

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello client.")?;
        let bytes = writer.into_inner().into();
        s_cipher = send(&mut server, &bytes, s_cipher, Compression::fast()).await?;

        let (reader, c) = recv(&mut client, c_cipher).await?;
        let mut reader = reader.reader(); c_cipher = c;
        let message = reader.read_string()?;
        assert_eq!("hello client.", message);

        let _ = s_cipher;
        let _ = c_cipher;
        Ok(())
    }
}
