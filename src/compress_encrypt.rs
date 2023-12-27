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
//!
//! This protocol is like this:
//! ```text
//!         ┌────┬────────┬────────────┐ (It may not be in contiguous memory.)
//! in  --> │ ** │ ****** │ ********** │
//!         └────┴────────┴────────────┘
//!           │
//!           │─ DeflateEncoder
//!           v
//!         ┌────────────────────┐ (Compressed bytes. In contiguous memory.)
//!         │ ****************** │
//!         └────────────────────┘
//!           │
//!           │─ Encrypt in-place
//!           v
//!         ┌────────────────────┐ (Compressed and encrypted bytes.)
//! out <-- │ ****************** │
//!         └────────────────────┘
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

/// Init the client side in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with `tcp_handler::compress_encrypt::client_start`.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - Current version of your application.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress_encrypt::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     let cipher = client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
pub async fn client_init<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<RsaPrivateKey, StarterError> {
    let key = RsaPrivateKey::new(&mut RsaRng, 2048)?;
    let mut writer = write_head(stream, identifier, version, true, true).await?;
    writer.write_u8_vec(&key.n().to_bytes_le())?;
    writer.write_u8_vec(&key.e().to_bytes_le())?;
    write_packet(stream, &writer.into_inner().into()).await?;
    Ok(key)
}

/// Init the server side in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with `tcp_handler::compress_encrypt::server_start`.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - A prediction to determine whether the client version is allowed.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress_encrypt::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     let cipher = server_start(&mut server, s_init).await?;
///     // Now the server is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
///
/// You can get the client version from this function:
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress_encrypt::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let mut version = None;
///     let s_init = server_init(&mut server, &"test", |v| {
///         version = Some(v.to_string());
///         v == "0"
///     }).await;
///     let cipher = server_start(&mut server, s_init).await?;
///     let version = version.unwrap();
///     // Now the version is got.
///     # let _ = cipher;
///     # let _ = version;
///     Ok(())
/// }
/// ```
pub async fn server_init<R: AsyncReadExt + Unpin + Send, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<RsaPublicKey, StarterError> {
    let mut reader = read_head(stream, identifier, version, true, true).await?;
    let n = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let e = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let key = RsaPublicKey::new(n, e)?;
    Ok(key)
}

/// Make sure the server side is ready to use in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with `tcp_handler::compress_encrypt::server_init`.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `last` - The return value of `tcp_handler::compress_encrypt::server_init`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress_encrypt::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     let cipher = server_start(&mut server, s_init).await?;
///     // Now the server is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
#[inline]
pub async fn server_start<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, last: Result<RsaPublicKey, StarterError>) -> Result<AesCipher, StarterError> {
    crate::encrypt::server_start(stream, last).await
}

/// Make sure the client side is ready to use in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with `tcp_handler::compress_encrypt::client_init`.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `last` - The return value of `tcp_handler::compress_encrypt::client_init`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress_encrypt::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     let cipher = client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
#[inline]
pub async fn client_start<R: AsyncReadExt + Unpin + Send>(stream: &mut R, last: Result<RsaPrivateKey, StarterError>) -> Result<AesCipher, StarterError> {
    crate::encrypt::client_start(stream, last).await
}

/// Send message in compress_encrypt tcp-handler protocol.
///
/// You may use some crate to read and write data,
/// such as [`serde`](https://crates.io/crates/serde),
/// [`postcard`](https://crates.io/crates/postcard) and
/// [`variable-len-reader`](https://crates.io/crates/variable-len-reader).
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `message` - The message to send.
///  * `level` - The level of compression.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use bytes::{BufMut, BytesMut};
/// use flate2::Compression;
/// use tcp_handler::compress_encrypt::{client_init, client_start, send};
/// use tokio::net::TcpStream;
/// use variable_len_reader::VariableWritable;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     let mut cipher = client_start(&mut client, c_init).await?;
///
///     let mut writer = BytesMut::new().writer();
///     writer.write_string("hello server.")?;
///     cipher = send(&mut client, &writer.into_inner().into(), cipher, Compression::default()).await?;
///
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
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

/// Recv message in compress_encrypt tcp-handler protocol.
///
/// You may use some crate to read and write data,
/// such as [`serde`](https://crates.io/crates/serde),
/// [`postcard`](https://crates.io/crates/postcard) and
/// [`variable-len-reader`](https://crates.io/crates/variable-len-reader).
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use bytes::Buf;
/// use tcp_handler::compress_encrypt::{recv, server_init, server_start};
/// use tokio::net::TcpListener;
/// use variable_len_reader::VariableReadable;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     let mut cipher = server_start(&mut server, s_init).await?;
///
///     let (reader, c) = recv(&mut server, cipher).await?;
///     let mut reader = reader.reader(); cipher = c;
///     let _message = reader.read_string()?;
///     Ok(())
/// }
/// ```
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
    use crate::common::test::{create, test_incorrect};

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

    test_incorrect!(compress_encrypt);
}
