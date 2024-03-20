use tokio::io::{AsyncRead, AsyncWrite};
use crate::common::StarterError;
use crate::raw::{self, send, recv};
use crate::streams::impl_tcp_handler;

pub struct TcpServerHandlerRaw<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
    version: String,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpServerHandlerRaw<R, W> {
    pub async fn new<P: FnOnce(&str) -> bool>(mut reader: R, mut writer: W, identifier: &str, version_prediction: P, version: &str) -> Result<Self, StarterError> {
        let init = raw::server_init(&mut reader, identifier, version_prediction).await;
        let (_protocol_version, application_version) = raw::server_start(&mut writer, identifier, version, init).await?;
        Ok(Self { reader, writer, version: application_version })
    }
}

impl_tcp_handler!(server TcpServerHandlerRaw);


pub struct TcpClientHandlerRaw<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    reader: R,
    writer: W,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> TcpClientHandlerRaw<R, W> {
    async fn init(&mut self, identifier: &str, version: &str) -> Result<(), StarterError> {
        let init = raw::client_init(&mut self.writer, identifier, version).await;
        raw::client_start(&mut self.reader, init).await?;
        Ok(())
    }

    pub async fn new(reader: R, writer: W, identifier: &str, version: &str) -> Result<Self, StarterError> {
        let mut handler = Self { reader, writer };
        handler.init(identifier, version).await?;
        Ok(handler)
    }
}

impl_tcp_handler!(client TcpClientHandlerRaw);


#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::{Buf, BufMut};
    use tokio::{spawn, try_join};
    use variable_len_reader::{VariableReader, VariableWriter};

    use crate::streams::raw::{TcpClientHandlerRaw, TcpServerHandlerRaw};
    use crate::streams::tests::{check_send_recv, create};

    #[tokio::test(flavor = "multi_thread")]
    async fn connect() -> Result<()> {
        let (cr, cw, sr, sw) = create().await?;
        let server = spawn(TcpServerHandlerRaw::new(sr, sw, "test", |v| v == "0", "0"));
        let client = spawn(TcpClientHandlerRaw::new(cr, cw, "test", "0"));
        let (server, client) = try_join!(server, client)?;
        let (mut server, mut client) = (server?, client?);

        assert_eq!("0", server.get_client_version());
        check_send_recv!(server, client, "Hello tcp-handler raw.");
        check_send_recv!(client, server, "Hello tcp-handler raw.");
        Ok(())
    }
}
