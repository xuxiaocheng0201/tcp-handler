//! Common utilities for this crate.

use std::io::Error;
use aead::consts::U12;
use aead::Error as AesGcmError;
use aes_gcm::aes::Aes256;
use aes_gcm::aes::cipher::InvalidLength;
use aes_gcm::{AesGcm, Nonce};
use bytes::buf::{Reader, Writer};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use variable_len_reader::{VariableReadable, VariableWritable};
use crate::config::get_max_packet_size;

/// Error in send/recv message.
#[derive(Error, Debug)]
pub enum PacketError {
    /// The packet size is larger than the maximum allowed packet size.
    /// This is due to you sending too much data at once,
    /// resulting in triggering memory safety limit
    ///
    /// You can reduce the size of data sent each time.
    /// Or you can change the maximum packet size by call `tcp_handler::config::set_config`.
    #[error("Packet size {0} is larger than the maximum allowed packet size {1}.")]
    TooLarge(usize, usize),

    /// During io bytes.
    #[error("During io bytes.")]
    IO(#[from] Error),

    /// During encrypting/decrypting bytes.
    #[cfg(feature = "encrypt")]
    #[error("During encrypting/decrypting bytes.")]
    AES(#[from] AesGcmError)
}

#[inline]
fn check_bytes_len(len: usize) -> Result<(), PacketError> {
    let config = get_max_packet_size();
    if len > config { Err(PacketError::TooLarge(len, config)) } else { Ok(()) }
}

/// ```text
///   ┌─ Packet length (in varint)
///   │    ┌─ Packet message
///   v    v
/// ┌────┬────────┐
/// │ ** │ ****** │
/// └────┴────────┘
/// ```
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


/// Error in init/start protocol.
#[derive(Error, Debug)]
pub enum StarterError {
    /// Magic bytes isn't matched.
    /// Please confirm that you are connected to the correct address.
    #[error("Invalid stream. MAGIC is not matched.")]
    InvalidStream(),

    /// Incompatible tcp-handler protocol.
    /// Please check whether you use the same protocol between client and server.
    /// This error will be thrown in **server** side.
    #[error("Incompatible protocol. compression: {0}, encryption: {1}")]
    ClientInvalidProtocol(bool, bool),

    /// Invalid application identifier.
    /// Please confirm that you are connected to the correct application,
    /// or that there are no spelling errors in the server and client identifiers.
    /// This error will be thrown in **server** side.
    #[error("Invalid identifier. received: {0}")]
    ClientInvalidIdentifier(String),

    /// Invalid application version.
    /// This is usually caused by the low version of the client application.
    /// This error will be thrown in **server** side.
    #[error("Invalid version. received: {0}")]
    ClientInvalidVersion(String),

    /// Incompatible tcp-handler protocol.
    /// Please check whether you use the same protocol between client and server.
    /// This error will be thrown in **client** side.
    #[error("Incompatible protocol.")]
    ServerInvalidProtocol(),

    /// Invalid application identifier.
    /// Please confirm that you are connected to the correct application,
    /// or that there are no spelling errors in the server and client identifiers.
    /// This error will be thrown in **client** side.
    #[error("Invalid identifier.")]
    ServerInvalidIdentifier(),

    /// Invalid application version.
    /// This is usually caused by the low version of the client application.
    /// This error will be thrown in **client** side.
    #[error("Invalid version.")]
    ServerInvalidVersion(),

    /// During io bytes.
    #[error("During io bytes.")]
    IO(#[from] Error),

    /// During reading/writing packet.
    #[error("During reading/writing packet.")]
    Packet(#[from] PacketError),

    /// During generating/encrypting/decrypting rsa key.
    #[cfg(feature = "encrypt")]
    #[error("During generating/encrypting/decrypting rsa key.")]
    RSA(#[from] rsa::Error),

    /// During generating/encrypting/decrypting aes key.
    #[cfg(feature = "encrypt")]
    #[error("During generating/encrypting/decrypting aes key.")]
    AES(#[from] InvalidLength),
}

impl TryFrom<StarterError> for Error {
    type Error = StarterError;

    fn try_from(value: StarterError) -> Result<Self, Self::Error> {
        match value {
            StarterError::IO(e) => { Ok(e) }
            StarterError::Packet(p) => match p {
                PacketError::IO(e) => { Ok(e) }
                _ => { Err(p.into()) }
            }
            _ => Err(value)
        }
    }
}

/// The MAGIC is generated in j-shell environment:
/// ```java
/// var r = new Random("tcp-handler".hashCode());
/// r.nextInt(0, 255); r.nextInt(0, 255);
/// r.nextInt(0, 255); r.nextInt(0, 255);
/// ```
/// The last two bytes is the version of the tcp-handler protocol.
static MAGIC_BYTES: [u8; 6] = [208, 8, 166, 104, 0, 0];

#[cfg(feature = "encrypt")]
/// The cipher in encryption mode.
/// You **must** update this value after each call to the send/recv function.
pub type AesCipher = (AesGcm<Aes256, U12>, Nonce<U12>);

/// ```text
///    ┌─ Magic bytes
///    │    ┌─ Protocol number (compression and encryption)
///    │    │     ┌─ Application identifier
///    │    │     │     ┌─ Application version
///    v    v     v     v
/// ┌─────┬────┬─────┬─────┐
/// │ *** │ 01 │ *** │ *** │
/// └─────┴────┴─────┴─────┘
/// ```
#[inline]
pub(crate) async fn write_head<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str, compression: bool, encryption: bool) -> Result<Writer<BytesMut>, StarterError> {
    stream.write_more(&MAGIC_BYTES).await?;
    let mut writer = BytesMut::new().writer();
    writer.write_bools_2(compression, encryption)?;
    writer.write_string(identifier)?;
    writer.write_string(version)?;
    Ok(writer)
}

#[inline]
pub(crate) async fn read_head<R: AsyncReadExt + Unpin + Send, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P, compression: bool, encryption: bool) -> Result<Reader<BytesMut>, StarterError> {
    let mut magic = vec![0; MAGIC_BYTES.len()];
    stream.read_more(&mut magic).await?;
    if magic != MAGIC_BYTES { return Err(StarterError::InvalidStream()); }
    let mut reader = read_packet(stream).await?.reader();
    let (read_compression, read_encryption) = reader.read_bools_2()?;
    if read_compression != compression || read_encryption != encryption { return Err(StarterError::ClientInvalidProtocol(read_compression, read_encryption)); }
    let read_identifier = reader.read_string()?;
    if read_identifier != identifier { return Err(StarterError::ClientInvalidIdentifier(read_identifier)); }
    let read_version = reader.read_string()?;
    if !version(&read_version) { return Err(StarterError::ClientInvalidVersion(read_version)); }
    Ok(reader)
}

/// ```text
///    ┌─ State number (protocol, identifier and version)
///    v
/// ┌─────┐
/// │ 000 │
/// └─────┘
/// ```
#[inline]
pub(crate) async fn write_last<W: AsyncWriteExt + Unpin + Send, E>(stream: &mut W, last: Result<E, StarterError>) -> Result<E, StarterError> {
    match last {
        Err(e) => {
            match e {
                StarterError::ClientInvalidProtocol(_, _) => { stream.write_bools_3(false, false, false).await?; }
                StarterError::ClientInvalidIdentifier(_) => { stream.write_bools_3(true, false, false).await?; }
                StarterError::ClientInvalidVersion(_) => { stream.write_bools_3(true, true, false).await?; }
                _ => {}
            }
            #[cfg(feature = "auto_flush")]
            let _ = stream.flush().await; // Ignore error.
            return Err(e);
        }
        Ok(k) => {
            stream.write_bools_3(true, true, true).await?;
            Ok(k)
        }
    }
}

#[inline]
pub(crate) async fn read_last<R: AsyncReadExt + Unpin + Send, E>(stream: &mut R, last: Result<E, StarterError>) -> Result<E, StarterError> {
    let k = last?;
    let (state, identifier, version) = stream.read_bools_3().await?;
    if !state { return Err(StarterError::ServerInvalidProtocol()) }
    if !identifier { return Err(StarterError::ServerInvalidIdentifier()) }
    if !version { return Err(StarterError::ServerInvalidVersion()) }
    Ok(k)
}


#[cfg(test)]
pub(crate) mod test {
    use anyhow::Result;
    use tokio::net::{TcpListener, TcpStream};

    pub(crate) async fn create() -> Result<(TcpStream, TcpStream)> {
        let addr = "localhost:0";
        let server = TcpListener::bind(addr).await?;
        let client = TcpStream::connect(server.local_addr()?).await?;
        let (server, _) = server.accept().await?;
        Ok((client, server))
    }

    #[tokio::test]
    async fn get_version() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let mut version = None;
        let c = crate::raw::client_init(&mut client, &"a", &"1").await;
        let s = crate::raw::server_init(&mut server, &"a", |v| { version = Some(v.to_string()); v == "1" }).await;
        crate::raw::server_start(&mut server, s).await?;
        crate::raw::client_start(&mut client, c).await?;
        assert_eq!(Some("1"), version.as_deref());
        Ok(())
    }

    macro_rules! test_incorrect {
        ($protocol: ident) => {
            #[tokio::test]
            async fn incorrect() -> anyhow::Result<()> {
                let (mut client, mut server) = create().await?;
                crate::variable_len_reader::asynchronous::AsyncVariableWritable::write_string(&mut client, "Something incorrect.").await?;
                let s = crate::$protocol::server_init(&mut server, &"a", |v| v == "1").await;
                match crate::$protocol::server_start(&mut server, s).await {
                    Ok(_) => assert!(false), Err(e) => match e {
                        crate::common::StarterError::InvalidStream() => assert!(true),
                        _ => assert!(false),
                    }
                }
                Ok(())
            }

            #[tokio::test]
            async fn identifier() -> Result<()> {
                let (mut client, mut server) = create().await?;
                let c = crate::$protocol::client_init(&mut client, &"a", &"1").await;
                let s = crate::$protocol::server_init(&mut server, &"b", |v| v == "1").await;
                match crate::$protocol::server_start(&mut server, s).await {
                    Ok(_) => assert!(false), Err(e) => match e {
                        crate::common::StarterError::ClientInvalidIdentifier(i) => assert_eq!("a", &i),
                        _ => assert!(false),
                    }
                }
                match crate::$protocol::client_start(&mut client, c).await {
                    Ok(_) => assert!(false), Err(e) => match e {
                        crate::common::StarterError::ServerInvalidIdentifier() => assert!(true),
                        _ => assert!(false),
                    }
                }
                Ok(())
            }

            #[tokio::test]
            async fn version() -> Result<()> {
                let (mut client, mut server) = create().await?;
                let c = crate::$protocol::client_init(&mut client, &"a", &"1").await;
                let s = crate::$protocol::server_init(&mut server, &"a", |v| v == "2").await;
                match crate::$protocol::server_start(&mut server, s).await {
                    Ok(_) => assert!(false), Err(e) => match e {
                        crate::common::StarterError::ClientInvalidVersion(v) => assert_eq!("1", &v),
                        _ => assert!(false),
                    }
                }
                match crate::$protocol::client_start(&mut client, c).await {
                    Ok(_) => assert!(false), Err(e) => match e {
                        crate::common::StarterError::ServerInvalidVersion() => assert!(true),
                        _ => assert!(false),
                    }
                }
                Ok(())
            }
        };
    }
    pub(crate) use test_incorrect;
}
