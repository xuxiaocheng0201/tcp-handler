#![doc = include_str!("../README.md")]
#![allow(unused_imports)] // For features.

pub mod config;
mod packet;
mod starter;

#[cfg(feature = "encrypt")]
pub use crate::starter::AesCipher;

pub mod raw {
    //! Raw protocol. Without encryption and compression.
    //!
    //! # Examples
    //! ```rust
    //! use anyhow::Result;
    //! use bytes::{Buf, BufMut, BytesMut};
    //! use tcp_handler::raw::*;
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
    //!     send(&mut client, &writer.into_inner().into()).await?;
    //!
    //!     let mut reader = recv(&mut server).await?.reader();
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello server.", message);
    //!
    //!     let mut writer = BytesMut::new().writer();
    //!     writer.write_string("hello client.")?;
    //!     send(&mut server, &writer.into_inner().into()).await?;
    //!
    //!     let mut reader = recv(&mut client).await?.reader();
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello client.", message);
    //!
    //!     Ok(())
    //! }
    //! ```

    pub use crate::starter::client_init;
    pub use crate::starter::server_init;
    pub use crate::starter::server_start;
    pub use crate::starter::client_start;
    pub use crate::packet::send;
    pub use crate::packet::recv;
}

#[cfg(feature = "compression")]
pub mod compress {
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

    pub use crate::starter::client_init_with_compress as client_init;
    pub use crate::starter::server_init_with_compress as server_init;
    pub use crate::starter::server_start_with_compress as server_start;
    pub use crate::starter::client_start_with_compress as client_start;
    pub use crate::packet::send_with_compress as send;
    pub use crate::packet::recv_with_compress as recv;
}

#[cfg(feature = "encrypt")]
pub mod encrypt {
    //! Encryption protocol. Without compression.
    //!
    //! With encryption, you can keep the data safe from being intercepted by others.
    //!
    //! # Example
    //! ```rust
    //! use anyhow::Result;
    //! use bytes::{Buf, BufMut, BytesMut};
    //! use tcp_handler::encrypt::*;
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
    //!     let mut s_cipher = server_start(&mut server, s_init).await?;
    //!     let mut c_cipher = client_start(&mut client, c_init).await?;
    //!
    //!     let mut writer = BytesMut::new().writer();
    //!     writer.write_string("hello server.")?;
    //!     let bytes = writer.into_inner().into();
    //!     c_cipher = send(&mut client, &bytes, c_cipher).await?;
    //!
    //!     let (reader, s) = recv(&mut server, s_cipher).await?;
    //!     let mut reader = reader.reader(); s_cipher = s;
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello server.", message);
    //!
    //!     let mut writer = BytesMut::new().writer();
    //!     writer.write_string("hello client.")?;
    //!     let bytes = writer.into_inner().into();
    //!     s_cipher = send(&mut server, &bytes, s_cipher).await?;
    //!
    //!     let (reader, c) = recv(&mut client, c_cipher).await?;
    //!     let mut reader = reader.reader(); c_cipher = c;
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello client.", message);
    //!
    //!     # let _ = s_cipher;
    //!     # let _ = c_cipher;
    //!     Ok(())
    //! }
    //! ```

    pub use crate::starter::client_init_with_encrypt as client_init;
    pub use crate::starter::server_init_with_encrypt as server_init;
    pub use crate::starter::server_start_with_encrypt as server_start;
    pub use crate::starter::client_start_with_encrypt as client_start;
    pub use crate::packet::send_with_encrypt as send;
    pub use crate::packet::recv_with_encrypt as recv;
}

#[cfg(all(feature = "compression", feature = "encrypt"))]
pub mod compress_encrypt {
    //! Compression and encryption protocol.
    //!
    //! Recommended to use this protocol.
    //!
    //! # Example
    //! ```rust
    //! use anyhow::Result;
    //! use bytes::{Buf, BufMut, BytesMut};
    //! use flate2::Compression;
    //! use tcp_handler::compress_encrypt::*;
    //! use tokio::net::{TcpListener, TcpStream};
    //! use variable_len_reader::{VariableReadable, VariableWritable};
    //!
    //! #[tokio::main]
    //! async fn main() -> Result<()> {
    //!     let server = TcpListener::bind("localhost:0").await?;
    //!     let mut client = TcpStream::connect(server.local_addr()?).await?;
    //!     let (mut server, _) = server.accept().await?;
    //!     let level = Compression::default();
    //!
    //!     let c_init = client_init(&mut client, &"test", &"0").await;
    //!     let s_init = server_init(&mut server, &"test", |v| v == "0").await;
    //!     let mut s_cipher = server_start(&mut server, s_init).await?;
    //!     let mut c_cipher = client_start(&mut client, c_init).await?;
    //!
    //!     let mut writer = BytesMut::new().writer();
    //!     writer.write_string("hello server.")?;
    //!     let bytes = writer.into_inner().into();
    //!     c_cipher = send(&mut client, &bytes, c_cipher, level).await?;
    //!
    //!     let (reader, s) = recv(&mut server, s_cipher).await?;
    //!     let mut reader = reader.reader(); s_cipher = s;
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello server.", message);
    //!
    //!     let mut writer = BytesMut::new().writer();
    //!     writer.write_string("hello client.")?;
    //!     let bytes = writer.into_inner().into();
    //!     s_cipher = send(&mut server, &bytes, s_cipher, level).await?;
    //!
    //!     let (reader, c) = recv(&mut client, c_cipher).await?;
    //!     let mut reader = reader.reader(); c_cipher = c;
    //!     let message = reader.read_string()?;
    //!     assert_eq!("hello client.", message);
    //!
    //!     # let _ = s_cipher;
    //!     # let _ = c_cipher;
    //!     Ok(())
    //! }
    //! ```

    pub use crate::starter::client_init_with_compress_and_encrypt as client_init;
    pub use crate::starter::server_init_with_compress_and_encrypt as server_init;
    pub use crate::starter::server_start_with_compress_and_encrypt as server_start;
    pub use crate::starter::client_start_with_compress_and_encrypt as client_start;
    pub use crate::packet::send_with_compress_and_encrypt as send;
    pub use crate::packet::recv_with_compress_and_encrypt as recv;
}

pub extern crate bytes;
pub extern crate variable_len_reader;
#[cfg(feature = "compression")]
pub extern crate flate2;
#[cfg(feature = "encrypt")]
pub extern crate rsa;
#[cfg(feature = "encrypt")]
pub extern crate aead;
#[cfg(feature = "encrypt")]
pub extern crate aes_gcm;
#[cfg(feature = "encrypt")]
pub extern crate sha2;
