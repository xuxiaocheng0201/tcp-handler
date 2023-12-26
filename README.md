# Tcp-Handler

[![Crate](https://img.shields.io/crates/v/tcp-handler.svg)](https://crates.io/crates/tcp-handler)
[![GitHub last commit](https://img.shields.io/github/last-commit/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/commits/master)
[![GitHub issues](https://img.shields.io/github/issues-raw/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/issues)
[![GitHub pull requests](https://img.shields.io/github/issues-pr/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/pulls)
[![GitHub](https://img.shields.io/github/license/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/blob/master/LICENSE)

**Read this in other languages: [English](README.md), [简体中文](README_zh.md).**

# Description

More conveniently use TcpStream in `tokio` to transfer `bytes` data chunks.


# Features

* Based on [tokio](https://crates.io/crates/tokio) and [bytes](https://crates.io/crates/bytes).
* Support `ReadHalf` and `WriteHalf` of `tokio::net::TcpStream`.
* Support encryption ([rsa](https://crates.io/crates/rsa) and [aes](https://crates.io/crates/aes-gcm)).
* Support compression ([flate2](https://crates.io/crates/flate2)).


# Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
tcp-handler = "~0.3"
```


# Example

Directly transfer data. Without encryption and compression:

```rust
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tokio::net::{TcpListener, TcpStream};
use tcp_handler::raw::{client_init, client_start, recv, send, server_init, server_start};
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // Create tcp stream.
    let addr = "localhost:25564";
    let server = TcpListener::bind(addr).await?;
    let mut client = TcpStream::connect(addr).await?;
    let (mut server, _) = server.accept().await?;
    
    // Prepare the protocol of tcp-handler.
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    server_start(&mut server, server_init).await?;
    client_start(&mut client, client_init).await?;
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    send(&mut client, &writer.into_inner()).await?;
    
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
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // Create tcp stream.
    let addr = "localhost:25564";
    let server = TcpListener::bind(addr).await?;
    let mut client = TcpStream::connect(addr).await?;
    let (mut server, _) = server.accept().await?;
    
    // Prepare the protocol of tcp-handler and the ciphers.
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    let server_cipher = server_start(&mut server, server_init).await?;
    let client_cipher = client_start(&mut client, client_init).await?;
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    let client_cipher = send(&mut client, &writer.into_inner(), client_cipher).await?;
    
    // Receive.
    let (reader, server_cipher) = recv(&mut server, server_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello server.", message);
    
    // Send.
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello client.")?;
    let _server_cipher = send(&mut client, &writer.into_inner(), server_cipher).await?;
    
    // Receive.
    let (reader, _client_cipher) = recv(&mut server, client_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello client.", message);
    
    Ok(())
}
```

The transmission method for compressed messages is similar to the above two,
please use methods in `compress` and `compress_encrypt` mod.
