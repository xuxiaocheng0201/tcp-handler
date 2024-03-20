# Tcp处理

[![Crate](https://img.shields.io/crates/v/tcp-handler.svg)](https://crates.io/crates/tcp-handler)
[![GitHub last commit](https://img.shields.io/github/last-commit/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/commits/master)
[![GitHub issues](https://img.shields.io/github/issues-raw/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/issues)
[![GitHub pull requests](https://img.shields.io/github/issues-pr/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/pulls)
[![GitHub](https://img.shields.io/github/license/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/blob/master/LICENSE)

**其他语言版本：[English](README.md)，[简体中文](README_zh.md)。**

# 描述

更便捷地使用`tokio::net::TcpStream`传输`bytes::Bytes`数据块。

你需要一些额外的库来读写数据，比如
[serde](https://crates.io/crates/serde)，
[postcard](https://crates.io/crates/postcard) 或者
[variable-len-reader](https://crates.io/crates/variable-len-reader)。

如果需要更方便地构建你的TCP应用，请参考
[tcp-server](https://crates.io/crates/tcp-server) 和 [tcp-client](https://crates.io/crates/tcp-client)。


# 特点

* 基于 [tokio](https://crates.io/crates/tokio) 和 [bytes](https://crates.io/crates/bytes) 库。
* 支持 `tokio::net::TcpStream` 的 `ReadHalf` 和 `WriteHalf`。
  （事实上只需要 `AsyncRead`/`AsyncWrite` 和 `Unpin` 就行）
* 支持 `bytes::Buf`，故可以发送使用 `chain` 连接的储存在非连续内存中的数据块。
* 支持加密（[rsa](https://crates.io/crates/rsa) 和 [aes](https://crates.io/crates/aes-gcm)）。
* 支持压缩（[flate2](https://crates.io/crates/flate2)）。
* 完整的 [API文档](https://docs.rs/tcp-handler/) 和数据模型。


# 用法

将以下内容添加到你的`Cargo.toml`：

```toml
[dependencies]
tcp-handler = "~0.6"
```


# 提示

如果在debug模式中，使用加密模式的`client_init`速度极慢，
请在客户端侧的`Cargo.toml`中添加：

```toml
[profile.dev.package.num-bigint-dig]
opt-level = 3 # 加快rsa密钥生成
```

这是在[rsa](https://crates.io/crates/rsa)库中出现的问题。详见[讨论](https://github.com/RustCrypto/RSA/issues/29)。


# 示例

直接传输，不使用加密和压缩：

```rust
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::protocols::raw::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReader, VariableWriter};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立 TCP 连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // 准备 tcp-handler 协议
    let client_init = client_init(&mut client, "test", "0.0.0").await;
    let server_init = server_init(&mut server, "test", |v| v == "0.0.0").await;
    server_start(&mut server, "test", "0.0.0", server_init).await?;
    client_start(&mut client, client_init).await?;
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    send(&mut client, &mut writer.into_inner()).await?;
    
    // 接收
    let mut reader = recv(&mut server).await?.reader();
    let message = reader.read_string()?;
    assert_eq!("hello server.", message);
    
    Ok(())
}
```

使用加密传输数据：

```rust
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::protocols::encrypt::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReader, VariableWriter};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立 TCP 连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // 准备 tcp-handler 协议，并协商密钥
    let client_init = client_init(&mut client, "test", "0.0.0").await;
    let server_init = server_init(&mut server, "test", |v| v == "0.0.0").await;
    let (server_cipher, _protocol_version, _client_version) =
            server_start(&mut server, "test", "0.0.0", server_init).await?;
    let client_cipher = client_start(&mut client, client_init).await?;
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    send(&mut client, &mut writer.into_inner(), &client_cipher).await?;
    
    // 接收
    let mut reader = recv(&mut server, &server_cipher).await?.reader();
    let message = reader.read_string()?;
    assert_eq!("hello server.", message);
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello client.")?;
    send(&mut client, &mut writer.into_inner(), &server_cipher).await?;
    
    // 接收
    let mut reader = recv(&mut server, &client_cipher).await?.reader();
    let message = reader.read_string()?;
    assert_eq!("hello client.", message);
    
    Ok(())
}
```

压缩流的传输方法与上述两者类似，请使用`compress`和`compress_encrypt`mod中的方法。

发送不连续的内存块：

```rust
use anyhow::Result;
use bytes::{Buf, Bytes};
use tcp_handler::protocols::raw::{client_init, client_start, recv, send, server_init, server_start};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    let client_init = client_init(&mut client, "chain", "0").await;
    let server_init = server_init(&mut server, "chain", |v| v == "0").await;
    server_start(&mut server, "chain", 0, server_init).await?;
    client_start(&mut client, client_init).await?;

    // 使用chain
    let mut messages = Bytes::from("a").chain(Bytes::from("b")).chain(Bytes::from("c"));
    send(&mut client, &mut messages).await?;
    let message = recv(&mut server).await?;
    assert_eq!(b"abc", message.as_ref());
    Ok(())
}
```


# 协议版本

内部实现的协议版本。
请注意只有当服务端和客户端的协议版本相同时，
才可以建立正常的连接。

| crate version | protocol version |
|---------------|------------------|
| \>=0.6.0      | 1                |
| <0.6.0        | 0                |


# License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
