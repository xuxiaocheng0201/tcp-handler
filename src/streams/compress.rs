//! `TcpHandler` warps the [`crate::protocols::compress`] protocol.

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::protocols::common::{PacketError, StarterError};
use crate::protocols::compress::{self, send, recv};
use crate::streams::impl_tcp_handler;

/// The server side `TcpHandler` of the `compress` protocol.
#[derive(Debug)]
pub struct TcpServerHandlerCompress<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    version: String,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpServerHandlerCompress<R, W> {
    /// Create and init a new `TcpServerHandlerCompress`.
    pub async fn new<P: FnOnce(&str) -> bool>(mut reader: R, mut writer: W, identifier: &str, version_prediction: P, version: &str) -> Result<Self, StarterError> {
        let init = compress::server_init(&mut reader, identifier, version_prediction).await;
        let (_protocol_version, application_version) = compress::server_start(&mut writer, identifier, version, init).await?;
        Ok(Self { reader, writer, version: application_version })
    }

    /// Deconstruct the `TcpServerHandlerCompress` into the inner parts.
    #[inline]
    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }

    /// **Unsafe!!!**
    /// Construct the `TcpServerHandlerCompress` from the inner parts.
    #[inline]
    pub fn from_inner(reader: R, writer: W, version: String) -> Self {
        Self { reader, writer, version }
    }

    /// Send a message to the client.
    #[inline]
    pub async fn send<B: Buf>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message).await
    }

    /// Receive a message from the client.
    #[inline]
    pub async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader).await
    }
}

impl_tcp_handler!(server TcpServerHandlerCompress);


/// The client side `TcpHandler` of the `compress` protocol.
#[derive(Debug)]
pub struct TcpClientHandlerCompress<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpClientHandlerCompress<R, W> {
    /// Create and init a new `TcpClientHandlerCompress`.
    pub async fn new(mut reader: R, mut writer: W, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let init = compress::client_init(&mut writer, identifier, version).await;
        compress::client_start(&mut reader, init).await?;
        Ok(Self { reader, writer })
    }

    /// Deconstruct the `TcpClientHandlerCompress` into the inner parts.
    #[inline]
    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }

    /// **Unsafe!!!**
    /// Construct the `TcpClientHandlerCompress` from the inner parts.
    #[inline]
    pub fn from_inner(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    /// Send a message to the server.
    #[inline]
    pub async fn send<B: Buf>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message).await
    }

    /// Receive a message from the server.
    #[inline]
    pub async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader).await
    }
}

impl_tcp_handler!(client TcpClientHandlerCompress);


#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::{Buf, BufMut};
    use tokio::{spawn, try_join};
    use variable_len_reader::{VariableReader, VariableWriter};
    use crate::streams::compress::{TcpClientHandlerCompress, TcpServerHandlerCompress};
    use crate::streams::tests::{check_send_recv, create};

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (cr, cw, sr, sw) = create().await?;
        let server = spawn(TcpServerHandlerCompress::new(sr, sw, "test", |v| v == "0", "0"));
        let client = spawn(TcpClientHandlerCompress::new(cr, cw, "test", "0"));
        let (server, client) = try_join!(server, client)?;
        let (mut server, mut client) = (server?, client?);

        assert_eq!("0", server.get_client_version());
        check_send_recv!(server, client, "Hello tcp-handler compress.");
        check_send_recv!(client, server, "Hello tcp-handler compress.");
        Ok(())
    }
}
