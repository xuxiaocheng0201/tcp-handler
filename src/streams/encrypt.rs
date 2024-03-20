//! `TcpHandler` warps the [`crate::protocols::encrypt`] protocol.

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::protocols::common::{Cipher, PacketError, StarterError};
use crate::protocols::encrypt::{self, send, recv};
use crate::streams::impl_tcp_handler;

/// The server side `TcpHandler` of the `encrypt` protocol.
#[derive(Debug)]
pub struct TcpServerHandlerEncrypt<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    cipher: Cipher,
    version: String,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpServerHandlerEncrypt<R, W> {
    /// Create and init a new `TcpServerHandlerEncrypt`.
    pub async fn new<P: FnOnce(&str) -> bool>(mut reader: R, mut writer: W, identifier: &str, version_prediction: P, version: &str) -> Result<Self, StarterError> {
        let init = encrypt::server_init(&mut reader, identifier, version_prediction).await;
        let (cipher, _protocol_version, application_version) = encrypt::server_start(&mut writer, identifier, version, init).await?;
        Ok(Self { reader, writer, cipher, version: application_version })
    }

    /// Deconstruct the `TcpServerHandlerEncrypt` into the inner parts.
    #[inline]
    pub fn into_inner(self) -> (R, W, Cipher) {
        (self.reader, self.writer, self.cipher)
    }

    /// **Unsafe!!!**
    /// Construct the `TcpServerHandlerEncrypt` from the inner parts.
    #[inline]
    pub fn from_inner(reader: R, writer: W, cipher: Cipher, version: String) -> Self {
        Self { reader, writer, cipher, version }
    }

    /// Send a message to the client.
    #[inline]
    pub async fn send<B: Buf>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message, &self.cipher).await
    }

    /// Receive a message from the client.
    #[inline]
    pub async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader, &self.cipher).await
    }
}

impl_tcp_handler!(server TcpServerHandlerEncrypt);


/// The client side `TcpHandler` of the `encrypt` protocol.
#[derive(Debug)]
pub struct TcpClientHandlerEncrypt<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    cipher: Cipher,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpClientHandlerEncrypt<R, W> {
    /// Create and init a new `TcpClientHandlerEncrypt`.
    pub async fn new(mut reader: R, mut writer: W, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let init = encrypt::client_init(&mut writer, identifier, version).await;
        let cipher = encrypt::client_start(&mut reader, init).await?;
        Ok(Self { reader, writer, cipher })
    }

    /// Deconstruct the `TcpClientHandlerEncrypt` into the inner parts.
    #[inline]
    pub fn into_inner(self) -> (R, W, Cipher) {
        (self.reader, self.writer, self.cipher)
    }

    /// **Unsafe!!!**
    /// Construct the `TcpClientHandlerEncrypt` from the inner parts.
    #[inline]
    pub fn from_inner(reader: R, writer: W, cipher: Cipher) -> Self {
        Self { reader, writer, cipher }
    }

    /// Send a message to the server.
    #[inline]
    pub async fn send<B: Buf>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message, &self.cipher).await
    }

    /// Receive a message from the server.
    #[inline]
    pub async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader, &self.cipher).await
    }
}

impl_tcp_handler!(client TcpClientHandlerEncrypt);


#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::{Buf, BufMut};
    use tokio::{spawn, try_join};
    use variable_len_reader::{VariableReader, VariableWriter};
    use crate::streams::encrypt::{TcpClientHandlerEncrypt, TcpServerHandlerEncrypt};
    use crate::streams::tests::{check_send_recv, create};

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (cr, cw, sr, sw) = create().await?;
        let server = spawn(TcpServerHandlerEncrypt::new(sr, sw, "test", |v| v == "0", "0"));
        let client = spawn(TcpClientHandlerEncrypt::new(cr, cw, "test", "0"));
        let (server, client) = try_join!(server, client)?;
        let (mut server, mut client) = (server?, client?);

        assert_eq!("0", server.get_client_version());
        check_send_recv!(server, client, "Hello tcp-handler encrypt.");
        check_send_recv!(client, server, "Hello tcp-handler encrypt.");
        Ok(())
    }
}
