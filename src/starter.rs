use std::io::Error;
#[cfg(feature = "encrypt")]
use aes_gcm::{
    AeadCore, Aes256Gcm, AesGcm, KeyInit, Nonce,
    aead::generic_array::typenum::U12,
    aes::Aes256,
    aes::cipher::InvalidLength,
};
use bytes::{Buf, BufMut, BytesMut};
use bytes::buf::{Reader, Writer};
#[cfg(feature = "encrypt")]
use rsa::{
    Oaep, RsaPrivateKey, RsaPublicKey,
    rand_core::OsRng,
};
#[cfg(feature = "encrypt")]
use sha2::Sha512;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::asynchronous::{AsyncVariableReadable, AsyncVariableWritable};
use variable_len_reader::{VariableReadable, VariableWritable};
use crate::packet::{PacketError, read_packet, write_packet};

#[derive(Error, Debug)]
pub enum StarterError {
    #[error("Invalid stream. MAGIC is not matched.")]
    InvalidStream(),
    #[error("Current state connection is not supported. encryption: {0}")]
    ClientInvalidState(bool), // Throw in server side.
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
/// The last two bytes is the version of the protocol.
static MAGIC_BYTES: [u8; 6] = [208, 8, 166, 104, 0, 0];

#[inline]
async fn write_head<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<Writer<BytesMut>, StarterError> {
    stream.write_more(&MAGIC_BYTES).await?;
    let mut writer = BytesMut::new().writer();
    writer.write_bool(false)?;
    writer.write_string(identifier)?;
    writer.write_string(version)?;
    Ok(writer)
}
pub async fn client_init<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<(), StarterError> {
    let writer = write_head(stream, identifier, version).await?;
    write_packet(stream, &writer.into_inner()).await?;
    Ok(())
}
#[cfg(feature = "encrypt")]
pub async fn client_init_with_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<RsaPrivateKey, StarterError> {
    use rsa::traits::PublicKeyParts;
    let key = RsaPrivateKey::new(&mut OsRng, 2048)?;
    let mut writer = write_head(stream, identifier, version).await?;
    writer.write_u8_vec(&key.n().to_bytes_le())?;
    writer.write_u8_vec(&key.e().to_bytes_le())?;
    write_packet(stream, &writer.into_inner()).await?;
    Ok(key)
}

#[inline]
async fn read_head<R: AsyncReadExt + Unpin + Send, P: Fn(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<Reader<BytesMut>, StarterError> {
    let mut magic = vec![0; MAGIC_BYTES.len()];
    stream.read_more(&mut magic).await?;
    if magic != MAGIC_BYTES { return Err(StarterError::InvalidStream()); }
    let mut reader = read_packet(stream).await?.reader();
    let encrypted = reader.read_bool()?;
    if encrypted { return Err(StarterError::ClientInvalidState(true)); }
    let read_identifier = reader.read_string()?;
    if read_identifier != identifier { return Err(StarterError::ClientInvalidIdentifier(read_identifier)); }
    let read_version = reader.read_string()?;
    if !version(&read_version) { return Err(StarterError::ClientInvalidVersion(read_version)); }
    Ok(reader)
}
pub async fn server_init<R: AsyncReadExt + Unpin + Send, P: Fn(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<(), StarterError> {
    read_head(stream, identifier, version).await?;
    Ok(())
}
#[cfg(feature = "encrypt")]
pub async fn server_init_with_encrypt<R: AsyncReadExt + Unpin + Send, P: Fn(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<RsaPublicKey, StarterError> {
    let mut reader = read_head(stream, identifier, version).await?;
    let n = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let e = rsa::BigUint::from_bytes_le(&reader.read_u8_vec()?);
    let key = RsaPublicKey::new(n, e)?;
    Ok(key)
}

#[inline]
async fn write_last<W: AsyncWriteExt + Unpin + Send, E>(stream: &mut W, last: Result<E, StarterError>) -> Result<E, StarterError> {
    match last {
        Err(e) => {
            match e {
                StarterError::ClientInvalidState(_) => { stream.write_bools_3(false, false, false).await?; }
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
pub async fn server_start<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, last: Result<(), StarterError>) -> Result<(), StarterError> {
    write_last(stream, last).await?;
    #[cfg(feature = "auto_flush")]
    stream.flush().await?;
    Ok(())
}
#[cfg(feature = "encrypt")]
pub async fn server_start_with_encrypt<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, last: Result<RsaPublicKey, StarterError>) -> Result<(AesGcm<Aes256, U12>, Nonce<U12>), StarterError> {
    let rsa = write_last(stream, last).await?;
    let aes = Aes256Gcm::generate_key(&mut OsRng);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    assert_eq!(12, nonce.len());
    let encrypted_aes = rsa.encrypt(&mut OsRng, Oaep::new::<Sha512>(), &aes)?;
    let cipher = Aes256Gcm::new(&aes);
    let mut writer = BytesMut::new().writer();
    writer.write_u8_vec(&encrypted_aes)?;
    writer.write_more(&nonce)?;
    write_packet(stream, &writer.into_inner()).await?;
    Ok((cipher, nonce))
}

#[inline]
async fn read_last<R: AsyncReadExt + Unpin + Send, E>(stream: &mut R, last: Result<E, StarterError>) -> Result<E, StarterError> {
    let k = last?;
    let (state, identifier, version) = stream.read_bools_3().await?;
    if !state { return Err(StarterError::ServerInvalidState()) }
    if !identifier { return Err(StarterError::ServerInvalidIdentifier()) }
    if !version { return Err(StarterError::ServerInvalidVersion()) }
    Ok(k)
}
pub async fn client_start<R: AsyncReadExt + Unpin + Send>(stream: &mut R, last: Result<(), StarterError>) -> Result<(), StarterError> {
    read_last(stream, last).await
}
#[cfg(feature = "encrypt")]
pub async fn client_start_with_encrypt<R: AsyncReadExt + Unpin + Send>(stream: &mut R, last: Result<RsaPrivateKey, StarterError>) -> Result<(AesGcm<Aes256, U12>, Nonce<U12>), StarterError> {
    let rsa = read_last(stream, last).await?;
    let mut reader = read_packet(stream).await?.reader();
    let encrypted_aes = reader.read_u8_vec()?;
    let mut nonce = [0; 12];
    reader.read_more(&mut nonce)?;
    let aes = rsa.decrypt(Oaep::new::<Sha512>(), &encrypted_aes)?;
    let cipher = Aes256Gcm::new_from_slice(&aes)?;
    let nonce = Nonce::from(nonce);
    Ok((cipher, nonce))
}

#[cfg(test)]
mod test {
    use aes_gcm::aead::Aead;
    use anyhow::Result;
    use tokio::net::{TcpListener, TcpStream};
    use crate::starter::{client_init, client_init_with_encrypt, client_start, client_start_with_encrypt, server_init, server_init_with_encrypt, server_start, server_start_with_encrypt};

    async fn create() -> Result<(TcpStream, TcpStream)> {
        let addr = "localhost:25564";
        let server = TcpListener::bind(addr).await?;
        let client = TcpStream::connect(addr).await?;
        let (server, _) = server.accept().await?;
        Ok((client, server))
    }

    #[tokio::test]
    async fn connect() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init(&mut client, &"a", &"1").await;
        let s = server_init(&mut server, &"a", |v| v == "1").await;
        server_start(&mut server, s).await?;
        client_start(&mut client, c).await?;
        Ok(())
    }

    #[tokio::test]
    async fn connect_with_encrypt() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init_with_encrypt(&mut client, &"a", &"1").await;
        let s = server_init_with_encrypt(&mut server, &"a", |v| v == "1").await;
        let s = server_start_with_encrypt(&mut server, s).await?;
        let c = client_start_with_encrypt(&mut client, c).await?;
        // assert_eq!(s.0, c.0);
        assert_eq!(s.1, c.1);

        let message = "tester".as_bytes();
        assert_eq!(s.0.encrypt(&s.1, message), c.0.encrypt(&c.1, message));
        Ok(())
    }
}
