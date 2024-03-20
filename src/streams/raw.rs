use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::common::{PacketError, StarterError};
use crate::raw::*;
use crate::streams::TcpHandler;

pub struct TcpServerHandlerRaw<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    version: String,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpServerHandlerRaw<R, W> {
    pub fn get_client_version(&self) -> &str {
        &self.version
    }

    pub async fn new<P: FnOnce(&str) -> bool>(mut reader: R, mut writer: W, identifier: &str, version_prediction: P, version: &str) -> Result<Self, StarterError> {
        let init = server_init(&mut reader, identifier, version_prediction).await;
        let (_protocol_version, application_version) = server_start(&mut writer, identifier, version, init).await?;
        Ok(Self { reader, writer, version: application_version })
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> TcpHandler for TcpServerHandlerRaw<R, W> {
    async fn send<B: Buf + Send>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message).await
    }

    async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader).await
    }
}


pub struct TcpClientHandlerRaw<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpClientHandlerRaw<R, W> {
    async fn init(&mut self, identifier: &str, version: &str) -> Result<(), StarterError> {
        let init = client_init(&mut self.writer, identifier, version).await;
        client_start(&mut self.reader, init).await?;
        Ok(())
    }

    pub async fn new(reader: R, writer: W, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let mut handler = Self { reader, writer };
        handler.init(identifier, version).await?;
        Ok(handler)
    }
}

#[async_trait]
impl <R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> TcpHandler for TcpClientHandlerRaw<R, W> {
    async fn send<B: Buf + Send>(&mut self, message: &mut B) -> Result<(), PacketError> {
        send(&mut self.writer, message).await
    }

    async fn recv(&mut self) -> Result<BytesMut, PacketError> {
        recv(&mut self.reader).await
    }
}

#[cfg(feature = "stream_net")]
impl TcpClientHandlerRaw<tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>, tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>> {
    #[cfg_attr(docsrs, doc(cfg(feature = "stream_net")))]
    pub async fn connect<A: tokio::net::ToSocketAddrs>(addr: A, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        let reader = tokio::io::BufReader::new(reader);
        let writer = tokio::io::BufWriter::new(writer);
        Self::new(reader, writer, identifier, version).await
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::{Buf, BufMut};
    use tokio::{spawn, try_join};
    use variable_len_reader::{VariableReader, VariableWriter};
    use crate::streams::raw::{TcpClientHandlerRaw, TcpServerHandlerRaw};
    use crate::streams::TcpHandler;
    use crate::streams::tests::{check_send_recv, create};

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (cr, cw, sr, sw) = create().await?;
        let server = spawn(TcpServerHandlerRaw::new(sr, sw, "test", |v| v == "0", "0"));
        let client = spawn(TcpClientHandlerRaw::new(cr, cw, "test", "0"));
        let (server, client) = try_join!(server, client)?;
        let (mut server, mut client) = (server?, client?);

        check_send_recv!(server, client, "Hello tcp-handler raw.");
        check_send_recv!(client, server, "Hello tcp-handler raw.");
        Ok(())
    }
}
