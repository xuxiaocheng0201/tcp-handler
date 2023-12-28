# Tcp-Handler

[![Crate](https://img.shields.io/crates/v/tcp-handler.svg)](https://crates.io/crates/tcp-handler)
[![GitHub last commit](https://img.shields.io/github/last-commit/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/commits/master)
[![GitHub issues](https://img.shields.io/github/issues-raw/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/issues)
[![GitHub pull requests](https://img.shields.io/github/issues-pr/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/pulls)
[![GitHub](https://img.shields.io/github/license/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/blob/master/LICENSE)

**Read this in other languages: [English](README.md), [简体中文](README_zh.md).**

# Description

More conveniently use `tokio::net::TcpStream` to transfer `bytes::Bytes` data chunks.


# Features

* Based on [tokio](https://crates.io/crates/tokio) and [bytes](https://crates.io/crates/bytes).
* Support `ReadHalf` and `WriteHalf` of `tokio::net::TcpStream`.
* Support `bytes::Buf`. So you can send discontinuous data chunks by calling `chain`.
* Support encryption ([rsa](https://crates.io/crates/rsa) and [aes](https://crates.io/crates/aes-gcm)).
* Support compression ([flate2](https://crates.io/crates/flate2)).
* Complete API [document](https://docs.rs/tcp-handler/) and data model.


# Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
tcp-handler = "~0.5"
```


# Note

If `client_init` using encryption mode is extremely slow in debug mode,
please add this to your `Cargo.toml` in client side:

```toml
[profile.dev.package.num-bigint-dig]
opt-level = 3 # Speed up rsa key gen.
```

This is an [issue](https://github.com/RustCrypto/RSA/issues/29) in [rsa](https://crates.io/crates/rsa) crate.


# Example

Directly transfer data. Without encryption and compression:

```rust
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::raw::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // Create tcp stream.
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // Prepare the protocol of tcp-handler.
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    server_start(&mut server, server_init).await?;
    client_start(&mut client, client_init).await?;
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    send(&mut client, &mut writer.into_inner()).await?;
    
    // Receive.
    let mut reader = recv(&mut server).await?.reader();
    let message = reader.read_string()?;
    assert_eq!("hello server.", message);
    
    Ok(())
}
```

Transfer message with encrypted protocol:

```rust
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::encrypt::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // Create tcp stream.
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // Prepare the protocol of tcp-handler and the ciphers.
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    let server_cipher = server_start(&mut server, server_init).await?;
    let client_cipher = client_start(&mut client, client_init).await?;
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    let client_cipher = send(&mut client, &mut writer.into_inner(), client_cipher).await?;
    
    // Receive.
    let (reader, server_cipher) = recv(&mut server, server_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello server.", message);
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello client.")?;
    let _server_cipher = send(&mut client, &mut writer.into_inner(), server_cipher).await?;
    
    // Receive.
    let (reader, _client_cipher) = recv(&mut server, client_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello client.", message);
    
    Ok(())
}
```

The transmission method for compressed messages is similar to the above two,
please use methods in `compress` and `compress_encrypt` mod.

Send discontinuous data chunks:

```rust
use anyhow::Result;
use bytes::{Buf, Bytes};
use tcp_handler::raw::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    let client_init = client_init(&mut client, &"chain", &"0").await;
    let server_init = server_init(&mut server, &"chain", |v| v == "0").await;
    server_start(&mut server, server_init).await?;
    client_start(&mut client, client_init).await?;

    // Using chain
    let mut messages = Bytes::from("a").chain(Bytes::from("b")).chain(Bytes::from("c"));
    send(&mut client, &mut messages).await?;
    let message = recv(&mut server).await?;
    assert_eq!(b"abc", message.as_ref());
    Ok(())
}
```
