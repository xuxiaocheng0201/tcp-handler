# Tcp处理

[![Crate](https://img.shields.io/crates/v/tcp-handler.svg)](https://crates.io/crates/tcp-handler)
[![GitHub last commit](https://img.shields.io/github/last-commit/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/commits/master)
[![GitHub issues](https://img.shields.io/github/issues-raw/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/issues)
[![GitHub pull requests](https://img.shields.io/github/issues-pr/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/pulls)
[![GitHub](https://img.shields.io/github/license/xuxiaocheng0201/tcp-handler)](https://github.com/xuxiaocheng0201/tcp-handler/blob/master/LICENSE)

**其他语言版本：[English](README.md)，[简体中文](README_zh.md)。**

# 描述

更便捷地使用`tokio`的 Tcp流 传输`bytes`数据块。


# 特性

* 基于 [tokio](https://crates.io/crates/tokio) 和 [bytes](https://crates.io/crates/bytes) 库。
* 支持 `tokio::net::TcpStream` 的 `ReadHalf` 和 `WriteHalf`。
* 支持加密（[rsa](https://crates.io/crates/rsa) 和 [aes](https://crates.io/crates/aes-gcm)）。
* 支持压缩（[flate2](https://crates.io/crates/flate2)）。


# 用法

将以下内容添加到你的`Cargo.toml`：

```toml
[dependencies]
tcp-handler = "~0.3"
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
use tokio::net::{TcpListener, TcpStream};
use tcp_handler::raw::{client_init, client_start, recv, send, server_init, server_start};
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立 TCP 连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // 准备 tcp-handler 协议
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    server_start(&mut server, server_init).await?;
    client_start(&mut client, client_init).await?;
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    send(&mut client, &writer.into_inner().into()).await?;
    
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
use tcp_handler::encrypt::{client_init, client_start, recv, send, server_init, server_start};
use variable_len_reader::{VariableReadable, VariableWritable};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立 TCP 连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    // 准备 tcp-handler 协议，并协商密钥
    let client_init = client_init(&mut client, &"test", &"0.0.0").await;
    let server_init = server_init(&mut server, &"test", |v| v == "0.0.0").await;
    let server_cipher = server_start(&mut server, server_init).await?;
    let client_cipher = client_start(&mut client, client_init).await?;
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello server.")?;
    let client_cipher = send(&mut client, &writer.into_inner().into(), client_cipher).await?;
    
    // 接收
    let (reader, server_cipher) = recv(&mut server, server_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello server.", message);
    
    // 发送
    let mut writer = BytesMut::new().writer();
    writer.write_string("hello client.")?;
    let _server_cipher = send(&mut client, &writer.into_inner().into(), server_cipher).await?;
    
    // 接收
    let (reader, _client_cipher) = recv(&mut server, client_cipher).await?;
    let message = reader.reader().read_string()?;
    assert_eq!("hello client.", message);
    
    Ok(())
}
```

压缩流的传输方法与上述两者类似，请使用`compress`和`compress_encrypt`mod中的方法。
