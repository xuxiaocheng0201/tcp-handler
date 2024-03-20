//! Useful `TcpHandler`s

pub mod raw;

use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use crate::common::PacketError;

/// The basic handler trait, providing send and receive methods.
#[async_trait]
pub trait TcpHandler {
    async fn send<B: Buf + Send>(&mut self, message: &mut B) -> Result<(), PacketError>;
    async fn recv(&mut self) -> Result<BytesMut, PacketError>;

    async fn send_recv<B: Buf + Send>(&mut self, message: &mut B) -> Result<BytesMut, PacketError> {
        self.send(message).await?;
        self.recv().await
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::io::{AsyncRead, AsyncWrite, duplex, split};

    pub async fn create() -> Result<(impl AsyncRead + Unpin, impl AsyncWrite + Unpin, impl AsyncRead + Unpin, impl AsyncWrite + Unpin)> {
        let (client, server) = duplex(1024);
        let (cr, cw) = split(client);
        let (sr, sw) = split(server);
        Ok((cr, cw, sr, sw))
    }

    macro_rules! check_send_recv {
        ($sender: expr, $receiver: expr, $msg: literal) => { {
            let mut writer = ::bytes::BytesMut::new().writer();
            writer.write_string($msg)?;
            $sender.send(&mut writer.into_inner()).await?;

            let mut reader = $receiver.recv().await?.reader();
            let msg = reader.read_string()?;
            assert_eq!($msg, msg);
        } };
    }
    pub(crate) use check_send_recv;
}
