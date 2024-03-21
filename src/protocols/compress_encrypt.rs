//! Compression and encryption protocol.
//!
//! Recommended to use this protocol.
//!
//! # Example
//! ```rust
//! use anyhow::Result;
//! use bytes::{Buf, BufMut, BytesMut};
//! use tcp_handler::protocols::compress_encrypt::*;
//! use tokio::net::{TcpListener, TcpStream};
//! use variable_len_reader::{VariableReader, VariableWriter};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let server = TcpListener::bind("localhost:0").await?;
//!     let mut client = TcpStream::connect(server.local_addr()?).await?;
//!     let (mut server, _) = server.accept().await?;
//!
//!     let c_init = client_init(&mut client, "test", "0").await;
//!     let s_init = server_init(&mut server, "test", |v| v == "0").await;
//!     let (s_cipher, protocol_version, client_version) = server_start(&mut server, "test", "0", s_init).await?;
//!     let c_cipher = client_start(&mut client, c_init).await?;
//!     # let _ = protocol_version;
//!     # let _ = client_version;
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello server.")?;
//!     let mut bytes = writer.into_inner();
//!     send(&mut client, &mut bytes, &c_cipher).await?;
//!
//!     let mut reader = recv(&mut server, &s_cipher).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello server.", message);
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello client.")?;
//!     let mut bytes = writer.into_inner();
//!     send(&mut server, &mut bytes, &s_cipher).await?;
//!
//!     let mut reader = recv(&mut client, &c_cipher).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello client.", message);
//!
//!     Ok(())
//! }
//! ```
//!
//! The send protocol:
//! ```text
//!         ┌────┬────────┬────────────┐ (It may not be in contiguous memory.)
//! in  --> │ ** │ ****** │ ********** │
//!         └────┴────────┴────────────┘
//!           └─────┐
//!          +Nonce │
//!           │     │─ Chain
//!           v     v
//!         ┌─────┬────┬────────┬────────────┐ (Zero copy. Not in contiguous memory.)
//!         │ *** │ ** │ ****** │ ********** │
//!         └─────┴────┴────────┴────────────┘
//!           │
//!           │─ DeflateEncoder
//!           v
//!         ┌─────────────────────┐ (Compressed bytes. In contiguous memory.)
//!         │ ******************* │
//!         └─────────────────────┘
//!           │
//!           │─ Encrypt in-place
//!           v
//!         ┌─────────────────────┐ (Compressed and encrypted bytes.)
//! out <-- │ ******************* │
//!         └─────────────────────┘
//! ```
//! The recv process:
//! ```text
//!         ┌─────────────────────┐ (Packet data.)
//! in  --> │ ******************* │
//!         └─────────────────────┘
//!           │
//!           │─ Decrypt in-place
//!           v
//!         ┌─────────────────────┐ (Decrypted bytes.)
//!         │ ******************* │
//!         └─────────────────────┘
//!           │
//!           │─ DeflateEncoder
//!           v
//!         ┌─────┬──────────────────┐ (Decrypted and decompressed bytes.)
//!         │ *** │ **************** │
//!         └─────┴──────────────────┘
//!           │     │
//!          -Nonce │
//! out <--  ───────┘
//! ```

use bytes::{Buf, BufMut, BytesMut};
use flate2::write::{DeflateDecoder, DeflateEncoder};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::block_in_place;
use variable_len_reader::{AsyncVariableReader, AsyncVariableWriter};
use variable_len_reader::helper::{AsyncReaderHelper, AsyncWriterHelper};
use crate::config::get_compression;
use crate::protocols::common::*;

/// Init the client side in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with [`client_start`].
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - Current version of your application.
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::protocols::compress_encrypt::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, "test", "0").await;
///     let cipher = client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
pub async fn client_init<W: AsyncWrite + Unpin>(stream: &mut W, identifier: &str, version: &str) -> Result<rsa::RsaPrivateKey, StarterError> {
    let (key, n, e) = block_in_place(|| generate_rsa_private())?;
    write_head(stream, ProtocolVariant::CompressEncryption, identifier, version).await?;
    AsyncWriterHelper(stream).help_write_u8_vec(&n).await?;
    AsyncWriterHelper(stream).help_write_u8_vec(&e).await?;
    flush(stream).await?;
    Ok(key)
}

/// Init the server side in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with [`server_start`].
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - A prediction to determine whether the client version is allowed.
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::protocols::compress_encrypt::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, "test", |v| v == "0").await;
///     let (cipher, protocol_version, client_version) = server_start(&mut server, "test", "0", s_init).await?;
///     // Now the server is ready to use.
///     # let _ = cipher;
///     # let _ = protocol_version;
///     # let _ = client_version;
///     Ok(())
/// }
/// ```
pub async fn server_init<R: AsyncRead + Unpin, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<((u16, String), rsa::RsaPublicKey), StarterError> {
    let versions = read_head(stream, ProtocolVariant::CompressEncryption, identifier, version).await?;
    let n = AsyncReaderHelper(stream).help_read_u8_vec().await?;
    let e = AsyncReaderHelper(stream).help_read_u8_vec().await?;
    let key = block_in_place(move || compose_rsa_public(n, e))?;
    Ok((versions, key))
}

/// Make sure the client side is ready to use in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with [`client_init`].
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `last` - The return value of [`client_init`].
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::protocols::compress_encrypt::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, "test", "0").await;
///     let cipher = client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     # let _ = cipher;
///     Ok(())
/// }
/// ```
pub async fn client_start<R: AsyncRead + Unpin>(stream: &mut R, last: Result<rsa::RsaPrivateKey, StarterError>) -> Result<Cipher, StarterError> {
    let rsa = read_last(stream, last).await?;
    let encrypted_aes = AsyncReaderHelper(stream).help_read_u8_vec().await?;
    let mut nonce = [0; 12];
    stream.read_more(&mut nonce).await?;
    let cipher = block_in_place(move || {
        use aes_gcm::aead::KeyInit;
        let aes = rsa.decrypt(rsa::Oaep::new::<rsa::sha2::Sha512>(), &encrypted_aes)?;
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&aes).unwrap();
        Ok::<_, StarterError>((cipher, aes_gcm::Nonce::from(nonce)))
    })?;
    Ok(Cipher::new(cipher))
}

/// Make sure the server side is ready to use in tcp-handler compress_encrypt protocol.
///
/// Must be used in conjunction with [`server_init`].
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `identifier` - The returned application identifier.
/// (Should be same with the para in [`server_init`].)
///  * `version` - The returned recommended application version.
/// (Should be passed the prediction in [`server_init`].)
///  * `last` - The return value of [`server_init`].
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::protocols::compress_encrypt::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, "test", |v| v == "0").await;
///     let (cipher, protocol_version, client_version) = server_start(&mut server, "test", "0", s_init).await?;
///     // Now the server is ready to use.
///     # let _ = cipher;
///     # let _ = protocol_version;
///     # let _ = client_version;
///     Ok(())
/// }
/// ```
pub async fn server_start<W: AsyncWrite + Unpin>(stream: &mut W, identifier: &str, version: &str, last: Result<((u16, String), rsa::RsaPublicKey), StarterError>) -> Result<(Cipher, u16, String), StarterError> {
    let ((va, vb), rsa) = write_last(stream, ProtocolVariant::CompressEncryption, identifier, version, last).await?;
    let (cipher, nonce, encrypted_aes) = block_in_place(move || {
        use aes_gcm::aead::{KeyInit, AeadCore};
        let aes = aes_gcm::Aes256Gcm::generate_key(&mut rand::thread_rng());
        let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut rand::thread_rng());
        debug_assert_eq!(12, nonce.len());
        let encrypted_aes = rsa.encrypt(&mut rand::thread_rng(), rsa::oaep::Oaep::new::<rsa::sha2::Sha512>(), &aes)?;
        let cipher = aes_gcm::Aes256Gcm::new(&aes);
        Ok::<_, StarterError>((cipher, nonce, encrypted_aes))
    })?;
    AsyncWriterHelper(stream).help_write_u8_vec(&encrypted_aes).await?;
    stream.write_more(&nonce).await?;
    flush(stream).await?;
    Ok((Cipher::new((cipher, nonce)), va, vb))
}

/// Send the message in tcp-handler compress_encrypt protocol.
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `message` - The message to send.
///  * `cipher` - The cipher returned from [`server_start`] or [`client_start`].
///
/// # Example
/// ```rust,no_run
/// # use anyhow::Result;
/// # use bytes::{BufMut, BytesMut};
/// # use tcp_handler::protocols::compress_encrypt::{client_init, client_start};
/// use tcp_handler::protocols::compress_encrypt::send;
/// # use tokio::net::TcpStream;
/// # use variable_len_reader::VariableWriter;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// #     let mut client = TcpStream::connect("localhost:25564").await?;
/// #     let c_init = client_init(&mut client, "test", "0").await;
/// #     let cipher = client_start(&mut client, c_init).await?;
/// let mut writer = BytesMut::new().writer();
/// writer.write_string("hello server.")?;
/// send(&mut client, &mut writer.into_inner(), &cipher).await?;
/// #     Ok(())
/// # }
/// ```
pub async fn send<W: AsyncWrite + Unpin, B: Buf>(stream: &mut W, message: &mut B, cipher: &Cipher) -> Result<(), PacketError> {
    let level = get_compression();
    let mut bytes = block_in_place(|| {
        use aes_gcm::aead::{AeadCore, AeadMutInPlace};
        use variable_len_reader::VariableWritable;
        let new_nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut rand::thread_rng());
        debug_assert_eq!(12, new_nonce.len());
        let mut encoder = DeflateEncoder::new(BytesMut::new().writer(), level);
        encoder.write_more(&new_nonce)?;
        encoder.write_more_buf(message)?;
        let mut bytes = encoder.finish()?.into_inner();
        let ((mut cipher, nonce), lock) = Cipher::get(cipher)?;
        cipher.encrypt_in_place(&nonce, &[], &mut bytes)?;
        Cipher::reset(lock, (cipher, new_nonce));
        Ok::<_, PacketError>(bytes)
    })?;
    write_packet(stream, &mut bytes).await?;
    flush(stream).await?;
    Ok(())
}

/// Recv the message in tcp-handler compress_encrypt protocol.
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `cipher` - The cipher returned from [`server_start`] or [`client_start`].
///
/// # Example
/// ```rust,no_run
/// # use anyhow::Result;
/// # use bytes::Buf;
/// # use tcp_handler::protocols::compress_encrypt::{server_init, server_start};
/// use tcp_handler::protocols::compress_encrypt::recv;
/// # use tokio::net::TcpListener;
/// # use variable_len_reader::VariableReader;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// #     let server = TcpListener::bind("localhost:25564").await?;
/// #     let (mut server, _) = server.accept().await?;
/// #     let s_init = server_init(&mut server, "test", |v| v == "0").await;
/// #     let (cipher, _, _) = server_start(&mut server, "test", "0", s_init).await?;
/// let mut reader = recv(&mut server, &cipher).await?.reader();
/// let message = reader.read_string()?;
/// #     let _ = message;
/// #     Ok(())
/// # }
/// ```
pub async fn recv<R: AsyncRead + Unpin>(stream: &mut R, cipher: &Cipher) -> Result<BytesMut, PacketError> {
    let mut buffer = read_packet(stream).await?;
    let message = block_in_place(move || {
        use aes_gcm::aead::AeadMutInPlace;
        use variable_len_reader::{VariableReadable, VariableWritable};
        let ((mut cipher, nonce), lock) = Cipher::get(cipher)?;
        cipher.decrypt_in_place(&nonce, &[], &mut buffer)?;
        let mut decoder = DeflateDecoder::new(BytesMut::new().writer());
        decoder.write_more_buf(&mut buffer)?;
        let mut reader = decoder.finish()?.into_inner().reader();
        let mut new_nonce = [0; 12];
        reader.read_more(&mut new_nonce)?;
        let new_nonce = aes_gcm::Nonce::from(new_nonce);
        Cipher::reset(lock, (cipher, new_nonce));
        Ok::<_, PacketError>(reader.into_inner())
    })?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use variable_len_reader::{VariableReader, VariableWriter};
    use crate::protocols::common::tests::create;
    use crate::protocols::compress_encrypt::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init(&mut client, "a", "1").await;
        let s = server_init(&mut server, "a", |v| v == "1").await;
        let (s_cipher, _, _) = server_start(&mut server, "a", "1", s).await?;
        let c_cipher = client_start(&mut client, c).await?;
for _ in 0..10 {
        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server in encrypt.")?;
        send(&mut client, &mut writer.into_inner(), &c_cipher).await?;

        let mut reader = recv(&mut server, &s_cipher).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello server in encrypt.", message);

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello client in encrypt.")?;
        send(&mut server, &mut writer.into_inner(), &s_cipher).await?;

        let mut reader = recv(&mut client, &c_cipher).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello client in encrypt.", message);
}
        Ok(())
    }
}
