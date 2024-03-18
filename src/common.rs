//! Common utilities for this crate.

use std::io::{Cursor, Error, Read, Write};
use bytes::Buf;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::config::get_max_packet_size;

/// Error in send/recv message.
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

/// Error in init/start protocol.
#[derive(Error, Debug)]
pub enum StarterError {
    /// [MAGIC_BYTES] isn't matched. Or the [MAGIC_VERSION] is no longer supported.
    /// Please confirm that you are connected to the correct address.
    #[error("Invalid stream. MAGIC is not matched.")]
    InvalidStream(),

    /// Incompatible tcp-handler protocol.
    /// Please check whether you use the same protocol between client and server.
    /// This error will be thrown in **server** side.
    #[error("Incompatible protocol. received protocol: {0:?}")]
    ClientInvalidProtocol(ProtocolVariant),

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
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    #[error("During generating/encrypting/decrypting rsa key.")]
    RSA(#[from] rsa::Error),

    /// During generating/encrypting/decrypting aes key.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    #[error("During generating/encrypting/decrypting aes key.")]
    AES(#[from] aes_gcm::aes::cipher::InvalidLength),
}

impl TryFrom<StarterError> for Error {
    type Error = StarterError;

    fn try_from(value: StarterError) -> Result<Self, Self::Error> {
        if let StarterError::IO(e) = value {
            return Ok(e);
        }
        if let StarterError::Packet(PacketError::IO(e)) = value {
            return Ok(e);
        }
        Err(value)
    }
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
/// | ------------ | ------------  |
/// | 0            | <=0.5.2       |
static MAGIC_VERSION: u16 = 0;

/// The variants of the protocol.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProtocolVariant {
    /// See [crate::raw].
    Raw,
    /// See [crate::compress].
    #[cfg(feature = "compression")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
    Compression,
    /// See [crate::encrypt].
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    Encryption,
    /// See [crate::compress_encrypt].
    #[cfg(feature = "compression_encrypt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compression_encrypt")))]
    CompressEncryption,
}

impl From<[bool; 2]> for ProtocolVariant {
    fn from(value: [bool; 2]) -> Self {
        match value {
            [true, false] => ProtocolVariant::Raw,
            #[cfg(feature = "compression")]
            [false, true] => ProtocolVariant::Compression,
            #[cfg(feature = "encryption")]
            [true, false] => ProtocolVariant::Encryption,
            #[cfg(feature = "compression_encrypt")]
            [true, true] => ProtocolVariant::CompressEncryption,
        }
    }
}

impl From<ProtocolVariant> for [bool; 2] {
    fn from(value: ProtocolVariant) -> Self {
        match value {
            ProtocolVariant::Raw => [true, false],
            #[cfg(feature = "compression")]
            ProtocolVariant::Compression => [false, true],
            #[cfg(feature = "encryption")]
            ProtocolVariant::Encryption => [true, false],
            #[cfg(feature = "compression_encrypt")]
            ProtocolVariant::CompressEncryption => [true, true],
        }
    }
}

/// ```text
///   ┌─ Magic bytes
///   │     ┌─ Magic version
///   │     │   ┌─ Protocol variant
///   v     v   v
/// ┌─────┬────┬────┐
/// │ *** │ ** │ ** │
/// └─────┴────┴────┘
/// ```
#[inline]
pub(crate) async fn write_head<W: AsyncWrite + Unpin>(stream: &mut W, protocol: ProtocolVariant) -> Result<(), StarterError> {
    use variable_len_reader::asynchronous::writer::AsyncVariableWriter;
    stream.write_more(&MAGIC_BYTES).await?;
    stream.write_u16_raw_be(MAGIC_VERSION).await?;
    stream.write_bools_2(protocol.into()).await?;
    Ok(())
}

/// See [write_head].
#[inline]
pub(crate) async fn read_head<R: AsyncRead + Unpin>(stream: &mut R, protocol: ProtocolVariant) -> Result<u16, StarterError> {
    use variable_len_reader::asynchronous::reader::AsyncVariableReader;
    let mut magic = [0; 4];
    stream.read_more(&mut magic).await?;
    if magic != MAGIC_BYTES { return Err(StarterError::InvalidStream()); }
    let version = stream.read_u16_raw_be().await?;
    if version != MAGIC_VERSION { return Err(StarterError::InvalidStream()); }
    let protocol_read = stream.read_bools_2().await?.into();
    if protocol_read != protocol { return Err(StarterError::ClientInvalidProtocol(protocol_read)); }
    Ok(version)
}


/// The cipher in encryption mode.
/// You **must** update this value after each call to the send/recv function.
#[cfg(feature = "encryption")]
pub(crate) type InnerAesCipher = (aes_gcm::Aes256Gcm, aes_gcm::Nonce<aes_gcm::aead::consts::U12>);


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
    use variable_len_reader::asynchronous::writer::AsyncVariableWriter;
    check_bytes_len(bytes.remaining())?;
    stream.write_usize_varint_ap(bytes.remaining()).await?;
    stream.write_more_buf(bytes).await?;
    Ok(())
}

/// See [write_packet].
pub(crate) async fn read_packet<R: AsyncRead + Unpin>(stream: &mut R) -> Result<impl Buf, PacketError> {
    use variable_len_reader::asynchronous::reader::AsyncVariableReader;
    let len = stream.read_usize_varint_ap().await?;
    check_bytes_len(len)?;
    let mut buf = vec![0; len];
    stream.read_more_buf(&mut buf).await?;
    Ok(Cursor::new(buf))
}


/// The cipher in encryption mode.
#[cfg(feature = "encryption")]
#[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
pub struct Cipher {
    cipher: tokio::sync::Mutex<Option<InnerAesCipher>>,
}

#[cfg(feature = "encryption")]
impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cipher")
            .field("cipher", self.cipher.try_lock()
                .map_or_else(|_| &"<locked>",
                             |inner| if (*inner).is_some() { &"<unlocked>" } else { &"<broken>" }))
            .finish()
    }
}

#[cfg(feature = "encryption")]
impl Cipher {
    pub(crate) fn new(cipher: InnerAesCipher) -> Self {
        Self {
            cipher: tokio::sync::Mutex::new(Some(cipher))
        }
    }

    pub(crate) async fn get<'a>(&'a self) -> Result<(InnerAesCipher, tokio::sync::MutexGuard<Option<InnerAesCipher>>), PacketError> {
        let mut guard = self.cipher.lock().await;
        let cipher = (*guard).take().ok_or(PacketError::Broken())?;
        Ok((cipher, guard))
    }

    #[inline]
    pub(crate) fn reset(mut guard: tokio::sync::MutexGuard<Option<InnerAesCipher>>, cipher: InnerAesCipher) {
        (*guard).replace(cipher);
    }
}


/// ```text
///   ┌─ Application identifier
///   │       ┌─ Application version
///   v       v
/// ┌───────┬───────┐
/// │ ***** │ ***** │
/// └───────┴───────┘
#[inline]
pub(crate) fn write_application<W: Write>(buffer: &mut W, identifier: &str, version: &str) -> Result<(), StarterError> {
    use variable_len_reader::synchronous::writer::VariableWriter;
    buffer.write_string(identifier)?;
    buffer.write_string(version)?;
    Ok(())
}

/// See [write_application].
#[inline]
pub(crate) fn read_application<R: Read, P: FnOnce(&str) -> bool>(buffer: &mut R, identifier: &str, version: P) -> Result<String, StarterError> {
    use variable_len_reader::synchronous::reader::VariableReader;
    let identifier_read = buffer.read_string()?;
    if identifier_read != identifier { return Err(StarterError::ClientInvalidIdentifier(identifier_read)); }
    let version_read = buffer.read_string()?;
    if !version(&version_read) { return Err(StarterError::ClientInvalidVersion(version_read)); }
    Ok(version_read)
}


/// ```text
///   ┌─ State bytes
///   v
/// ┌─────┐
/// │ *** │
/// └─────┘
/// ```
#[inline]
pub(crate) fn write_last<W: Write, E>(buffer: &mut W, last: Result<E, StarterError>) -> Result<E, StarterError> {
    use variable_len_reader::synchronous::writer::VariableWriter;
    match last {
        Err(e) => {
            match e { // FIXME: report the current protocol and update the protocol version.
                StarterError::ClientInvalidProtocol(_) => { buffer.write_bools_3([false, false, false])?; }
                StarterError::ClientInvalidIdentifier(_) => { buffer.write_bools_3([true, false, false])?; }
                StarterError::ClientInvalidVersion(_) => { buffer.write_bools_3([true, true, false])?; }
                _ => {}
            }
            return Err(e);
        }
        Ok(k) => {
            buffer.write_bools_3([true, true, true])?;
            Ok(k)
        }
    }
}

#[inline]
pub(crate) fn read_last<R: Read, E>(buffer: &mut R, last: Result<E, StarterError>) -> Result<E, StarterError> {
    use variable_len_reader::synchronous::reader::VariableReader;
    let extra = last?;
    let [state, identifier, version] = buffer.read_bools_3()?;
    if !state { return Err(StarterError::ServerInvalidProtocol()) }
    if !identifier { return Err(StarterError::ServerInvalidIdentifier()) }
    if !version { return Err(StarterError::ServerInvalidVersion()) }
    Ok(extra)
}
