//! Useful `TcpHandler`s

pub mod raw;
pub mod compress;

use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use crate::common::PacketError;

/// The basic handler trait, providing send and receive methods.
#[async_trait]
pub trait TcpHandler {
    async fn handler_send<B: Buf + Send>(&mut self, message: &mut B) -> Result<(), PacketError>;
    async fn handler_recv(&mut self) -> Result<BytesMut, PacketError>;

    async fn handler_send_recv<B: Buf + Send>(&mut self, message: &mut B) -> Result<BytesMut, PacketError> {
        self.handler_send(message).await?;
        self.handler_recv().await
    }
}

macro_rules! impl_tcp_handler {
    (@ $struct: ident) => {
        impl<R: ::tokio::io::AsyncRead + Unpin, W: ::tokio::io::AsyncWrite + Unpin> $struct<R, W> {
            pub async fn send<B: ::bytes::Buf>(&mut self, message: &mut B) -> Result<(), $crate::protocols::common::PacketError> {
                send(&mut self.writer, message).await
            }

            pub async fn recv(&mut self) -> Result<::bytes::BytesMut, $crate::protocols::common::PacketError> {
                recv(&mut self.reader).await
            }
        }

        #[::async_trait::async_trait]
        impl<R: ::tokio::io::AsyncRead + Unpin + Send, W: ::tokio::io::AsyncWrite + Unpin + Send> $crate::streams::TcpHandler for $struct<R, W> {
            async fn handler_send<B: ::bytes::Buf + Send>(&mut self, message: &mut B) -> Result<(), $crate::protocols::common::PacketError> {
                self.send(message).await
            }

            async fn handler_recv(&mut self) -> Result<::bytes::BytesMut, $crate::protocols::common::PacketError> {
                self.recv().await
            }
        }
    };
    (server $server: ident) => {
        impl_tcp_handler!(@ $server);

        impl<R: ::tokio::io::AsyncRead + Unpin, W: ::tokio::io::AsyncWrite + Unpin> $server<R, W> {
            pub fn get_client_version(&self) -> &str {
                &self.version
            }
        }
    };
    (client $client: ident) => {
        impl_tcp_handler!(@ $client);

        #[cfg(feature = "stream_net")]
        impl $client<::tokio::io::BufReader<::tokio::net::tcp::OwnedReadHalf>, ::tokio::io::BufWriter<::tokio::net::tcp::OwnedWriteHalf>> {
            #[cfg_attr(docsrs, doc(cfg(feature = "stream_net")))]
            #[doc(concat!("Connection to `addr`, and construct the `", stringify!($client), "` using [", , stringify!($client), "::new]."))]
            pub async fn connect<A: ::tokio::net::ToSocketAddrs>(addr: A, identifier: &str, version: &str) -> Result<Self, $crate::protocols::common::StarterError> {
                let stream = ::tokio::net::TcpStream::connect(addr).await?;
                let (reader, writer) = stream.into_split();
                let reader = ::tokio::io::BufReader::new(reader);
                let writer = ::tokio::io::BufWriter::new(writer);
                Self::new(reader, writer, identifier, version).await
            }
        }
    }
}
use impl_tcp_handler;

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
