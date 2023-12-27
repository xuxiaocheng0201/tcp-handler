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


#[derive(Error, Debug)]
pub enum StarterError {
    #[error("Invalid stream. MAGIC is not matched.")]
    InvalidStream(),
    #[error("Current state connection is not supported. compression: {0}, encryption: {1}")]
    ClientInvalidState(bool, bool), // Throw in server side.
    #[error("Invalid identifier. received: {0}")]
    ClientInvalidIdentifier(String), // Throw in server side.
    #[error("Invalid version. received: {0}")]
    ClientInvalidVersion(String), // Throw in server side.
    #[error("Current state connection is not supported.")]
    ServerInvalidState(), // Throw in client side.
    #[error("Invalid identifier.")]
    ServerInvalidIdentifier(), // Throw in client side.
    #[error("Invalid version.")]
    ServerInvalidVersion(), // Throw in client side.
    #[error("During io bytes.")]
    IO(#[from] Error),
    #[error("During reading/writing packet.")]
    Packet(#[from] PacketError),
    #[cfg(feature = "encrypt")]
    #[error("During generating/encrypting/decrypting rsa key.")]
    RSA(#[from] rsa::Error),
    #[cfg(feature = "encrypt")]
    #[error("During generating/encrypting/decrypting aes key.")]
    AES(#[from] InvalidLength),
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
pub type AesCipher = (AesGcm<Aes256, U12>, Nonce<U12>);

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
    if read_compression != compression || read_encryption != encryption { return Err(StarterError::ClientInvalidState(read_compression, read_encryption)); }
    let read_identifier = reader.read_string()?;
    if read_identifier != identifier { return Err(StarterError::ClientInvalidIdentifier(read_identifier)); }
    let read_version = reader.read_string()?;
    if !version(&read_version) { return Err(StarterError::ClientInvalidVersion(read_version)); }
    Ok(reader)
}

#[inline]
pub(crate) async fn write_last<W: AsyncWriteExt + Unpin + Send, E>(stream: &mut W, last: Result<E, StarterError>) -> Result<E, StarterError> {
    match last {
        Err(e) => {
            match e {
                StarterError::ClientInvalidState(_, _) => { stream.write_bools_3(false, false, false).await?; }
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
    if !state { return Err(StarterError::ServerInvalidState()) }
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

    // TODO: incorrect connection and error handle.
}
