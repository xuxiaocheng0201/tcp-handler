//! Compression protocol. Without encryption.
//!
//! With compression, you can reduce the size of the data sent by the server and the client.
//!
//! # Example
//! ```rust
//! use anyhow::Result;
//! use bytes::{Buf, BufMut, BytesMut};
//! use flate2::Compression;
//! use tcp_handler::compress::*;
//! use tokio::net::{TcpListener, TcpStream};
//! use variable_len_reader::{VariableReadable, VariableWritable};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let server = TcpListener::bind("localhost:0").await?;
//!     let mut client = TcpStream::connect(server.local_addr()?).await?;
//!     let (mut server, _) = server.accept().await?;
//!
//!     let c_init = client_init(&mut client, &"test", &"0").await;
//!     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
//!     server_start(&mut server, s_init).await?;
//!     client_start(&mut client, c_init).await?;
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello server.")?;
//!     let bytes = writer.into_inner().into();
//!     send(&mut client, &bytes, Compression::default()).await?;
//!
//!     let mut reader = recv(&mut server).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello server.", message);
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello client.")?;
//!     let bytes = writer.into_inner().into();
//!     send(&mut server, &bytes, Compression::default()).await?;
//!
//!     let mut reader = recv(&mut client).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello client.", message);
//!
//!     Ok(())
//! }
//! ```

use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::Compression;
use flate2::write::{DeflateDecoder, DeflateEncoder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use variable_len_reader::VariableWritable;
use crate::common::{PacketError, read_head, read_packet, StarterError, write_head, write_packet};

/// Init the client side in tcp-handler compress protocol.
///
/// Must be used in conjunction with `tcp_handler::compress::client_start`.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - Current version of your application.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     Ok(())
/// }
/// ```
pub async fn client_init<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, identifier: &str, version: &str) -> Result<(), StarterError> {
    let writer = write_head(stream, identifier, version, true, false).await?;
    write_packet(stream, &writer.into_inner().into()).await?;
    Ok(())
}

/// Init the server side in tcp-handler compress protocol.
///
/// Must be used in conjunction with `tcp_handler::compress::server_start`.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - A prediction to determine whether the client version is allowed.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     server_start(&mut server, s_init).await?;
///     // Now the server is ready to use.
///     Ok(())
/// }
/// ```
///
/// You can get the client version from this function:
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{server_init, server_start};
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
///     server_start(&mut server, s_init).await?;
///     let version = version.unwrap();
///     // Now the version is got.
///     Ok(())
/// }
/// ```
pub async fn server_init<R: AsyncReadExt + Unpin + Send, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<(), StarterError> {
    read_head(stream, identifier, version, true, false).await?;
    Ok(())
}

/// Make sure the server side is ready to use in tcp-handler compress protocol.
///
/// Must be used in conjunction with `tcp_handler::compress::server_init`.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `last` - The return value of `tcp_handler::compress::server_init`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     server_start(&mut server, s_init).await?;
///     // Now the server is ready to use.
///     Ok(())
/// }
/// ```
#[inline]
pub async fn server_start<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, last: Result<(), StarterError>) -> Result<(), StarterError> {
    crate::raw::server_start(stream, last).await
}

/// Make sure the client side is ready to use in tcp-handler compress protocol.
///
/// Must be used in conjunction with `tcp_handler::compress::client_init`.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `last` - The return value of `tcp_handler::compress::client_init`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     Ok(())
/// }
/// ```
#[inline]
pub async fn client_start<R: AsyncReadExt + Unpin + Send>(stream: &mut R, last: Result<(), StarterError>) -> Result<(), StarterError> {
    crate::raw::client_start(stream, last).await
}

/// Send message in compress tcp-handler protocol.
///
/// You may use some crate to read and write data,
/// such as [`serde`](https://crates.io/crates/serde),
/// [`postcard`](https://crates.io/crates/postcard) and
/// [`variable-len-reader`](https://crates.io/crates/variable-len-reader)
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
/// use tcp_handler::compress::{client_init, client_start, send};
/// use tokio::net::TcpStream;
/// use variable_len_reader::VariableWritable;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, &"test", &"0").await;
///     client_start(&mut client, c_init).await?;
///
///     let mut writer = BytesMut::new().writer();
///     writer.write_string("hello server.")?;
///     send(&mut client, &writer.into_inner().into(), Compression::default()).await?;
///     Ok(())
/// }
/// ```
pub async fn send<W: AsyncWriteExt + Unpin + Send>(stream: &mut W, message: &Bytes, level: Compression) -> Result<(), PacketError> {
    let mut message = message.clone();
    let mut encoder = DeflateEncoder::new(BytesMut::new().writer(), level);
    while message.has_remaining() {
        let len = encoder.write_more(message.chunk())?;
        message.advance(len);
    }
    let bytes = encoder.finish()?.into_inner();
    write_packet(stream, &bytes.into()).await
}

/// Recv message in compress tcp-handler protocol.
///
/// You may use some crate to read and write data,
/// such as [`serde`](https://crates.io/crates/serde),
/// [`postcard`](https://crates.io/crates/postcard) and
/// [`variable-len-reader`](https://crates.io/crates/variable-len-reader)
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///
/// # Example
/// ```no_run
/// use anyhow::Result;
/// use bytes::Buf;
/// use tcp_handler::compress::{recv, server_init, server_start};
/// use tokio::net::TcpListener;
/// use variable_len_reader::VariableReadable;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
///     server_start(&mut server, s_init).await?;
///
///     let mut reader = recv(&mut server).await?.reader();
///     let _message = reader.read_string()?;
///     Ok(())
/// }
/// ```
pub async fn recv<R: AsyncReadExt + Unpin + Send>(stream: &mut R) -> Result<BytesMut, PacketError> {
    let mut bytes = read_packet(stream).await?;
    let mut decoder = DeflateDecoder::new(BytesMut::new().writer());
    while bytes.has_remaining() {
        let len = decoder.write_more(bytes.chunk())?;
        bytes.advance(len);
    }
    Ok(decoder.finish()?.into_inner())
}


#[cfg(test)]
mod test {
    use anyhow::Result;
    use bytes::{Buf, BufMut, BytesMut};
    use flate2::Compression;
    use variable_len_reader::{VariableReadable, VariableWritable};
    use crate::compress::{recv, send};
    use crate::common::test::create;

    #[tokio::test]
    async fn connect() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = crate::compress::client_init(&mut client, &"a", &"1").await;
        let s = crate::compress::server_init(&mut server, &"a", |v| v == "1").await;
        crate::compress::server_start(&mut server, s).await?;
        crate::compress::client_start(&mut client, c).await?;

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        send(&mut client, &writer.into_inner().into(), Compression::best()).await?;

        let mut reader = recv(&mut server).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello server.", message);

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello client.")?;
        send(&mut server, &writer.into_inner().into(), Compression::fast()).await?;

        let mut reader = recv(&mut client).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello client.", message);

        Ok(())
    }
}
