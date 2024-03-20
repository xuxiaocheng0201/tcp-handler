//! Compression protocol. Without encryption.
//!
//! With compression, you can reduce the size of the data sent by the server and the client.
//!
//! Set the compression level by calling [`tcp_handler::config::set_config`].
//!
//! # Example
//! ```rust
//! use anyhow::Result;
//! use bytes::{Buf, BufMut, BytesMut};
//! use tcp_handler::compress::*;
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
//!     server_start(&mut server, "test", "0", s_init).await?;
//!     client_start(&mut client, c_init).await?;
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello server.")?;
//!     let mut bytes = writer.into_inner();
//!     send(&mut client, &mut bytes).await?;
//!
//!     let mut reader = recv(&mut server).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello server.", message);
//!
//!     let mut writer = BytesMut::new().writer();
//!     writer.write_string("hello client.")?;
//!     let mut bytes = writer.into_inner();
//!     send(&mut server, &mut bytes).await?;
//!
//!     let mut reader = recv(&mut client).await?.reader();
//!     let message = reader.read_string()?;
//!     assert_eq!("hello client.", message);
//!
//!     Ok(())
//! }
//! ```
//!
//! The send process:
//! ```text
//!         ┌────┬────────┬────────────┐ (It may not be in contiguous memory.)
//! in  --> │ ** │ ****** │ ********** │
//!         └────┴────────┴────────────┘
//!           │
//!           │─ DeflateEncoder
//!           v
//!         ┌──────────────────┐ (Compressed bytes. In contiguous memory.)
//! out <-- │ **************** │
//!         └──────────────────┘
//! ```
//! The recv process:
//! ```text
//!         ┌──────────────────┐ (Packet data.)
//! in  --> │ **************** │
//!         └──────────────────┘
//!           │
//!           │─ DeflateDecoder
//!           v
//!         ┌────────────────────┐ (Decompressed bytes.)
//! out <-- │ ****************** │
//!         └────────────────────┘
//! ```

use bytes::{Buf, BufMut, BytesMut};
use flate2::write::{DeflateDecoder, DeflateEncoder};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::block_in_place;
use crate::common::*;
use crate::config::get_compression;

/// Init the client side in tcp-handler compress protocol.
///
/// Must be used in conjunction with [`client_start`].
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `identifier` - The identifier of your application.
///  * `version` - Current version of your application.
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, "test", "0").await;
///     client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     Ok(())
/// }
/// ```
#[inline]
pub async fn client_init<W: AsyncWrite + Unpin>(stream: &mut W, identifier: &str, version: &str) -> Result<(), StarterError> {
    write_head(stream, ProtocolVariant::Compression, identifier, version).await?;
    flush(stream).await?;
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
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, "test", |v| v == "0").await;
///     server_start(&mut server, "test", "0", s_init).await?;
///     // Now the server is ready to use.
///     Ok(())
/// }
/// ```
#[inline]
pub async fn server_init<R: AsyncRead + Unpin, P: FnOnce(&str) -> bool>(stream: &mut R, identifier: &str, version: P) -> Result<(u16, String), StarterError> {
    read_head(stream, ProtocolVariant::Compression, identifier, version).await
}

/// Make sure the client side is ready to use in tcp-handler compress protocol.
///
/// Must be used in conjunction with [`client_init`].
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///  * `last` - The return value of [`client_init`].
///
/// # Example
/// ```rust,no_run
/// use anyhow::Result;
/// use tcp_handler::compress::{client_init, client_start};
/// use tokio::net::TcpStream;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut client = TcpStream::connect("localhost:25564").await?;
///     let c_init = client_init(&mut client, "test", "0").await;
///     client_start(&mut client, c_init).await?;
///     // Now the client is ready to use.
///     Ok(())
/// }
/// ```
#[inline]
pub async fn client_start<R: AsyncRead + Unpin>(stream: &mut R, last: Result<(), StarterError>) -> Result<(), StarterError> {
    read_last(stream, last).await
}

/// Make sure the server side is ready to use in tcp-handler compress protocol.
///
/// Must be used in conjunction with [`server_init`].
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
/// use tcp_handler::compress::{server_init, server_start};
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = TcpListener::bind("localhost:25564").await?;
///     let (mut server, _) = server.accept().await?;
///     let s_init = server_init(&mut server, "test", |v| v == "0").await;
///     let (protocol_version, client_version) = server_start(&mut server, "test", "0", s_init).await?;
///     // Now the server is ready to use.
///     # let _ = protocol_version;
///     # let _ = client_version;
///     Ok(())
/// }
/// ```
#[inline]
pub async fn server_start<W: AsyncWrite + Unpin>(stream: &mut W, identifier: &str, version: &str, last: Result<(u16, String), StarterError>) -> Result<(u16, String), StarterError> {
    let res = write_last(stream, ProtocolVariant::Compression, identifier, version, last).await?;
    flush(stream).await?;
    Ok(res)
}

/// Send the message in compress tcp-handler protocol.
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `WriteHalf`.
///  * `message` - The message to send.
///
/// # Example
/// ```rust,no_run
/// # use anyhow::Result;
/// # use bytes::{BufMut, BytesMut};
/// # use tcp_handler::compress::{client_init, client_start};
/// use tcp_handler::compress::send;
/// # use tokio::net::TcpStream;
/// # use variable_len_reader::VariableWriter;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// #     let mut client = TcpStream::connect("localhost:25564").await?;
/// #     let c_init = client_init(&mut client, "test", "0").await;
/// #     client_start(&mut client, c_init).await?;
/// let mut buffer = BytesMut::new().writer();
/// buffer.write_string("hello server!")?;
/// send(&mut client, &mut buffer.into_inner()).await?;
/// #     Ok(())
/// # }
/// ```
pub async fn send<W: AsyncWrite + Unpin, B: Buf>(stream: &mut W, message: &mut B) -> Result<(), PacketError> {
    let level = get_compression();
    let mut bytes = block_in_place(move || {
        use variable_len_reader::VariableWritable;
        let mut encoder = DeflateEncoder::new(BytesMut::new().writer(), level);
        encoder.write_more_buf(message)?;
        Ok::<_, PacketError>(encoder.finish()?.into_inner())
    })?;
    write_packet(stream, &mut bytes).await?;
    flush(stream).await?;
    Ok(())
}

/// Recv the message in compress tcp-handler protocol.
///
/// # Runtime
/// Due to call [`block_in_place`] internally,
/// this function cannot be called in a `current_thread` runtime.
///
/// # Arguments
///  * `stream` - The tcp stream or `ReadHalf`.
///
/// # Example
/// ```rust,no_run
/// # use anyhow::Result;
/// # use bytes::Buf;
/// # use tcp_handler::compress::{server_init, server_start};
/// use tcp_handler::compress::recv;
/// # use tokio::net::TcpListener;
/// # use variable_len_reader::VariableReader;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// #     let server = TcpListener::bind("localhost:25564").await?;
/// #     let (mut server, _) = server.accept().await?;
/// #     let s_init = server_init(&mut server, "test", |v| v == "0").await;
/// #     server_start(&mut server, "test", "0", s_init).await?;
/// let mut reader = recv(&mut server).await?.reader();
/// let message = reader.read_string()?;
/// #     let _ = message;
/// #     Ok(())
/// # }
/// ```
pub async fn recv<R: AsyncRead + Unpin>(stream: &mut R) -> Result<BytesMut, PacketError> {
    let mut bytes = read_packet(stream).await?;
    let message = block_in_place(move || {
        use variable_len_reader::VariableWritable;
        let mut decoder = DeflateDecoder::new(BytesMut::new().writer());
        decoder.write_more_buf(&mut bytes)?;
        Ok::<_, PacketError>(decoder.finish()?.into_inner())
    })?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use variable_len_reader::{VariableReader, VariableWriter};
    use crate::common::tests::create;
    use crate::compress::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (mut client, mut server) = create().await?;
        let c = client_init(&mut client, "a", "1").await;
        let s = server_init(&mut server, "a", |v| v == "1").await;
        server_start(&mut server, "a", "1", s).await?;
        client_start(&mut client, c).await?;

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server in compress.")?;
        send(&mut client, &mut writer.into_inner()).await?;

        let mut reader = recv(&mut server).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello server in compress.", message);

        let mut writer = BytesMut::new().writer();
        writer.write_string("hello client in compress.")?;
        send(&mut server, &mut writer.into_inner()).await?;

        let mut reader = recv(&mut client).await?.reader();
        let message = reader.read_string()?;
        assert_eq!("hello client in compress.", message);

        Ok(())
    }
}
