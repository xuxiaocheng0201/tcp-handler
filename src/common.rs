//! Common utilities for this crate.

use std::io::{Cursor, Error};
use bytes::Buf;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use variable_len_reader::{AsyncVariableReader, AsyncVariableWriter};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use variable_len_reader::util::read_buf::OwnedReadBuf;
use crate::config::get_max_packet_size;

/// Error when send/recv packets.
#[derive(Error, Debug)]
pub enum PacketError {
    /// The packet size is larger than the maximum allowed packet size.
    /// This is due to you sending too much data at once,
    /// resulting in triggering memory safety limit.
    ///
    /// You can reduce the size of data packet sent each time.
    /// Or you can change the maximum packet size by call [tcp_handler::config::set_config].
    #[error("Packet size {0} is larger than the maximum allowed packet size {1}.")]
    TooLarge(usize, usize),

    /// During io bytes.
    #[error("During io bytes.")]
    IO(#[from] Error),

    /// During encrypting/decrypting bytes.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    #[error("During encrypting/decrypting bytes.")]
    AES(#[from] aes_gcm::aead::Error),

    /// Broken stream cipher. This is a fatal error.
    ///
    /// When another error returned during send/recv, the stream is broken because no [Cipher] received.
    /// In order not to panic, marks this stream as broken and returns this error.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    #[error("Broken stream.")]
    Broken(),
}

/// Error when init/start protocol.
#[derive(Error, Debug)]
pub enum StarterError {
    /// [MAGIC_BYTES] isn't matched. Or the [MAGIC_VERSION] is no longer supported.
    /// Please confirm that you are connected to the correct address.
    #[error("Invalid stream. MAGIC is not matched.")]
    InvalidStream(),

    /// Incompatible tcp-handler protocol.
    /// The param came from the other side.
    /// Please check whether you use the same protocol between client and server.
    #[error("Incompatible protocol. received protocol: {0:?}")]
    InvalidProtocol(ProtocolVariant),

    /// Invalid application identifier.
    /// The param came from the other side.
    /// Please confirm that you are connected to the correct application,
    /// or that there are no spelling errors in the server and client identifiers.
    #[error("Invalid identifier. received: {0}")]
    InvalidIdentifier(String),

    /// Invalid application version.
    /// The param came from the other side.
    /// This is usually caused by the low version of the client application.
    #[error("Invalid version. received: {0}")]
    InvalidVersion(String),

    /// During io bytes.
    #[error("During io bytes.")]
    IO(#[from] Error),

    /// During generating/encrypting/decrypting rsa key.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    #[error("During generating/encrypting/decrypting rsa key.")]
    RSA(#[from] rsa::Error),
}


/// The MAGIC is generated in j-shell environment:
/// ```java
/// var r = new Random("tcp-handler".hashCode());
/// r.nextInt(0, 255); r.nextInt(0, 255);
/// r.nextInt(0, 255); r.nextInt(0, 255);
/// ```
static MAGIC_BYTES: [u8; 4] = [208, 8, 166, 104];

/// The version of the tcp-handler protocol.
///
/// | version code | crate version |
/// | ------------ | ------------- |
/// | 1            | >=0.6.0       |
/// | 0            | <0.6.0        |
static MAGIC_VERSION: u16 = 1;

/// The variants of the protocol.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProtocolVariant {
    /// See [crate::raw].
    Raw,
    /// See [crate::compress].
    Compression,
    /// See [crate::encrypt].
    Encryption,
    /// See [crate::compress_encrypt].
    CompressEncryption,
}

impl From<[bool; 2]> for ProtocolVariant {
    fn from(value: [bool; 2]) -> Self {
        match value {
            [false, false] => ProtocolVariant::Raw,
            [false, true] => ProtocolVariant::Compression,
            [true, false] => ProtocolVariant::Encryption,
            [true, true] => ProtocolVariant::CompressEncryption,
        }
    }
}

impl From<ProtocolVariant> for [bool; 2] {
    fn from(value: ProtocolVariant) -> Self {
        match value {
            ProtocolVariant::Raw => [false, false],
            ProtocolVariant::Compression => [false, true],
            ProtocolVariant::Encryption => [true, false],
            ProtocolVariant::CompressEncryption => [true, true],
        }
    }
}

#[inline]
pub(crate) async fn write_u8_vec<W: AsyncWrite + Unpin>(stream: &mut W, vec: &[u8]) -> Result<(), <W as AsyncVariableWritable>::Error> {
    stream.write_usize_varint_ap(vec.len()).await?;
    stream.write_more(vec).await?;
    Ok(())
}

#[inline]
pub(crate) async fn write_string<W: AsyncWrite + Unpin>(stream: &mut W, string: &str) -> Result<(), <W as AsyncVariableWritable>::Error> {
    write_u8_vec(stream, string.as_bytes()).await
}

#[inline]
pub(crate) async fn read_u8_vec<R: AsyncRead + Unpin>(stream: &mut R) -> Result<Vec<u8>, <R as AsyncVariableReadable>::Error> {
    let len = stream.read_usize_varint_ap().await?;
    let mut buf = vec![0; len];
    stream.read_more(&mut buf).await?;
    Ok(buf)
}

#[inline]
pub(crate) async fn read_string<R: AsyncRead + Unpin>(stream: &mut R) -> Result<String, <R as AsyncVariableReadable>::Error> {
    let buf = read_u8_vec(stream).await?;
    String::from_utf8(buf).map_err(|e| R::read_string_error("ReadString", e))
}

/// In client side.
/// ```text
///   ┌─ Magic bytes
///   │     ┌─ Magic version
///   │     │    ┌─ Protocol variant
///   │     │    │    ┌─ Application identifier
///   │     │    │    │       ┌─ Application version
///   v     v    v    v       v
/// ┌─────┬────┬────┬───────┬───────┐
/// │ *** │ ** │ ** │ ***** │ ***** │
/// └─────┴────┴────┴───────┴───────┘
/// ```
pub(crate) async fn write_head<W: AsyncWrite + Unpin>(stream: &mut W, protocol: ProtocolVariant, identifier: &str, version: &str) -> Result<(), StarterError> {
    stream.write_more(&MAGIC_BYTES).await?;
    stream.write_u16_raw_be(MAGIC_VERSION).await?;
    stream.write_bools_2(protocol.into()).await?;
    write_string(stream, identifier).await?;
    write_string(stream, version).await?;
    Ok(())
}

/// In server side.
/// See [write_head].
pub(crate) async fn read_head<R: AsyncRead + Unpin, P: FnOnce(&str) -> bool>(stream: &mut R, protocol: ProtocolVariant, identifier: &str, version: P) -> Result<(u16, String), StarterError> {
    let mut magic = [0; 4];
    stream.read_more(&mut magic).await?;
    if magic != MAGIC_BYTES { return Err(StarterError::InvalidStream()); }
    let protocol_version = stream.read_u16_raw_be().await?;
    if protocol_version != MAGIC_VERSION { return Err(StarterError::InvalidStream()); }
    let protocol_read = stream.read_bools_2().await?.into();
    if protocol_read != protocol { return Err(StarterError::InvalidProtocol(protocol_read)); }
    let identifier_read = read_string(stream).await?;
    if identifier_read != identifier { return Err(StarterError::InvalidIdentifier(identifier_read)); }
    let version_read = read_string(stream).await?;
    if !version(&version_read) { return Err(StarterError::InvalidVersion(version_read)); }
    Ok((protocol_version, version_read))
}

/// In server side.
/// ```text
///   ┌─ State bytes
///   │   ┌─ Error information.
///   v   v
/// ┌───┬───────┐
/// │ * │ ***** │
/// └───┴───────┘
/// ```
pub(crate) async fn write_last<W: AsyncWrite + Unpin, E>(stream: &mut W, protocol: ProtocolVariant, identifier: &str, version: &str, last: Result<E, StarterError>) -> Result<E, StarterError> {
    match last {
        Err(e) => {
            match &e {
                StarterError::InvalidProtocol(_) => {
                    stream.write_bools_2([false, false]).await?;
                    stream.write_bools_2(protocol.into()).await?;
                }
                StarterError::InvalidIdentifier(_) => {
                    stream.write_bools_2([false, true]).await?;
                    write_string(stream, identifier).await?;
                }
                StarterError::InvalidVersion(_) => {
                    stream.write_bools_2([true, false]).await?;
                    write_string(stream, version).await?;
                }
                _ => {}
            }
            return Err(e);
        },
        Ok(k) => {
            stream.write_bools_2([true, true]).await?;
            Ok(k)
        }
    }
}

/// In client side.
/// See [write_last].
pub(crate) async fn read_last<R: AsyncRead + Unpin, E>(stream: &mut R, last: Result<E, StarterError>) -> Result<E, StarterError> {
    let extra = last?;
    match stream.read_bools_2().await? {
        [true, true] => Ok(extra),
        [false, false] => Err(StarterError::InvalidProtocol(stream.read_bools_2().await?.into())),
        [false, true] => Err(StarterError::InvalidIdentifier(read_string(stream).await?)),
        [true, false] => Err(StarterError::InvalidVersion(read_string(stream).await?)),
    }
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
pub(crate) async fn write_packet<W: AsyncWrite + Unpin, B: Buf>(stream: &mut W, bytes: &mut B) -> Result<(), PacketError> {
    check_bytes_len(bytes.remaining())?;
    stream.write_usize_varint_ap(bytes.remaining()).await?;
    stream.write_more_buf(bytes).await?;
    Ok(())
}

/// See [write_packet].
pub(crate) async fn read_packet<R: AsyncRead + Unpin>(stream: &mut R) -> Result<impl Buf + Send + Unpin, PacketError> {
    let len = stream.read_usize_varint_ap().await?;
    check_bytes_len(len)?;
    let mut buf = OwnedReadBuf::new(vec![0; len]);
    stream.read_more_buf(&mut buf).await?;
    let buf = Cursor::new(buf.into_inner());
    Ok(buf)
}


/// The cipher in encryption mode.
/// You **must** update this value after each call to the send/recv function.
#[cfg(feature = "encryption")]
pub(crate) type InnerAesCipher = (aes_gcm::Aes256Gcm, aes_gcm::Nonce<aes_gcm::aead::consts::U12>);

/// The cipher in encryption mode.
#[cfg(feature = "encryption")]
#[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
pub struct Cipher {
    cipher: std::sync::Mutex<Option<InnerAesCipher>>,
}

#[cfg(feature = "encryption")]
impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cipher")
            .field("cipher", &self.cipher.try_lock()
                .map_or_else(|_| "<locked>",
                             |inner| if (*inner).is_some() { "<unlocked>" } else { "<broken>" }))
            .finish()
    }
}

#[cfg(feature = "encryption")]
impl Cipher {
    #[inline]
    pub(crate) fn new(cipher: InnerAesCipher) -> Self {
        Self {
            cipher: std::sync::Mutex::new(Some(cipher))
        }
    }

    #[inline]
    pub(crate) fn get(&self) -> Result<(InnerAesCipher, std::sync::MutexGuard<Option<InnerAesCipher>>), PacketError> {
        let mut guard = self.cipher.lock().unwrap();
        let cipher = (*guard).take().ok_or(PacketError::Broken())?;
        Ok((cipher, guard))
    }

    #[inline]
    pub(crate) fn reset(mut guard: std::sync::MutexGuard<Option<InnerAesCipher>>, cipher: InnerAesCipher) {
        (*guard).replace(cipher);
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use anyhow::Result;
    use bytes::{Buf, Bytes};
    use tokio::io::{AsyncRead, AsyncWrite, duplex};
    use crate::common::{read_packet, write_packet};

    pub(crate) async fn create() -> Result<(impl AsyncRead + AsyncWrite + Unpin, impl AsyncRead + AsyncWrite + Unpin)> {
        let (client, server) = duplex(1024);
        Ok((client, server))
    }

    #[tokio::test]
    async fn packet() -> Result<()> {
        let (mut client, mut server) = create().await?;

        let source = &[1, 2, 3, 4, 5];
        write_packet(&mut client, &mut Bytes::from_static(source)).await?;
        let res = read_packet(&mut server).await?;
        assert_eq!(source, res.chunk());

        Ok(())
    }
}
