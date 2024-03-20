use tokio::io::{AsyncRead, AsyncWrite};
use crate::common::StarterError;
use crate::compress::{self, send, recv};
use crate::streams::impl_tcp_handler;

pub struct TcpServerHandlerCompress<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    version: String,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpServerHandlerCompress<R, W> {
    pub async fn new<P: FnOnce(&str) -> bool>(mut reader: R, mut writer: W, identifier: &str, version_prediction: P, version: &str) -> Result<Self, StarterError> {
        let init = compress::server_init(&mut reader, identifier, version_prediction).await;
        let (_protocol_version, application_version) = compress::server_start(&mut writer, identifier, version, init).await?;
        Ok(Self { reader, writer, version: application_version })
    }
}

impl_tcp_handler!(server TcpServerHandlerCompress);


pub struct TcpClientHandlerCompress<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpClientHandlerCompress<R, W> {
    async fn init(&mut self, identifier: &str, version: &str) -> Result<(), StarterError> {
        let init = compress::client_init(&mut self.writer, identifier, version).await;
        compress::client_start(&mut self.reader, init).await?;
        Ok(())
    }

    pub async fn new(reader: R, writer: W, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let mut handler = Self { reader, writer };
        handler.init(identifier, version).await?;
        Ok(handler)
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
