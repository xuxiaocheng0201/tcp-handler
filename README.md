# Tcp-Handler

[![Crate](https://img.shields.io/crates/v/tcp-handler.svg)](https://crates.io/crates/tcp-handler)
[![GitHub last commit](https://img.shields.io/github/last-commit/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/commits/master)
[![GitHub issues](https://img.shields.io/github/issues-raw/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/issues)
[![GitHub pull requests](https://img.shields.io/github/issues-pr/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/pulls)
[![GitHub](https://img.shields.io/github/license/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/blob/master/LICENSE)

**Read this in other languages: [English](README.md), [简体中文](README_zh.md).**

# Description

More conveniently use `tokio::net::TcpStream` to transfer `bytes::Bytes` data chunks.

You may use extra crate to read and write data,
such as [serde](https://crates.io/crates/serde),
[postcard](https://crates.io/crates/postcard) and
[variable-len-reader](https://crates.io/crates/variable-len-reader).

See [tcp-server](https://crates.io/crates/tcp-server) and [tcp-client](https://crates.io/crates/tcp-client)
for conveniently building your tcp application. 


# Features

* Based on [tokio](https://crates.io/crates/tokio) and [bytes](https://crates.io/crates/bytes).
* Support `ReadHalf` and `WriteHalf` of `tokio::net::TcpStream`.
  (In fact anything impl `AsyncRead`/`AsyncWrite` and `Unpin` can be used.)
* Support `bytes::Buf`. So you can send discontinuous data chunks by calling `chain`.
* Support encryption ([rsa](https://crates.io/crates/rsa) and [aes](https://crates.io/crates/aes-gcm)).
* Support compression ([flate2](https://crates.io/crates/flate2)).
* Complete API [document](https://docs.rs/tcp-handler/) and data model.


# Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
tcp-handler = "^1.0"
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

With `TcpHandler`, you can use all the protocols in a similar way.

```rust
use anyhow::{Error, Result};
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::raw::*;
use tokio::{spawn, try_join};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReader, VariableWriter};

#[tokio::main]
async fn main() -> Result<()> {
    // Create tcp stream.
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    let client = spawn(async move {
        let mut client = TcpClientHandlerRaw::from_stream(client, "YourApplication", "1.0.0").await?;

        // Send.
        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        client.send(&mut writer.into_inner()).await?;
      
        Ok::<_, Error>(())
    });
    let server = spawn(async move {
        let mut server = TcpServerHandlerRaw::from_stream(server, "YourApplication", |v| v == "1.0.0", "1.0.0").await?;
        assert_eq!(server.get_client_version(), "1.0.0");

        // Receive.
        let mut reader = server.recv().await?.reader();
        let res = reader.read_string()?;
        assert_eq!(res, "hello server.");

        Ok::<_, Error>(())
    });
    try_join!(client, server)?;
    Ok(())
}
```


# Benchmarks

Speed: `raw` > `compress` > `compress_encrypt` > `encrypt`

But currently the benchmarks are not serious. Welcome to contribute.


# Protocol Version

The protocol version code used internally.
Note only when the server and client sides have the same code,
they can connect normally.

| crate version | protocol version |
|---------------|------------------|
| \>=0.6.0      | 1                |
| <0.6.0        | 0                |


# License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
