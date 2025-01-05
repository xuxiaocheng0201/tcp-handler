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
tcp-handler = "^1.0"
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

通过 `TcpHandler`， 你可以以相似的方式使用各类协议：

```rust
use anyhow::{Error, Result};
use bytes::{Buf, BufMut, BytesMut};
use tcp_handler::raw::*;
use tokio::{spawn, try_join};
use tokio::net::{TcpListener, TcpStream};
use variable_len_reader::{VariableReader, VariableWriter};

#[tokio::main]
async fn main() -> Result<()> {
    // 建立 TCP 连接
    let server = TcpListener::bind("localhost:0").await?;
    let mut client = TcpStream::connect(server.local_addr()?).await?;
    let (mut server, _) = server.accept().await?;
    
    let client = spawn(async move {
        let mut client = TcpClientHandlerRaw::from_stream(client, "YourApplication", "1.0.0").await?;

        // 发送
        let mut writer = BytesMut::new().writer();
        writer.write_string("hello server.")?;
        client.send(&mut writer.into_inner()).await?;
      
        Ok::<_, Error>(())
    });
    let server = spawn(async move {
        let mut server = TcpServerHandlerRaw::from_stream(server, "YourApplication", |v| v == "1.0.0", "1.0.0").await?;
        assert_eq!(server.get_client_version(), "1.0.0");

        // 接收
        let mut reader = server.recv().await?.reader();
        let res = reader.read_string()?;
        assert_eq!(res, "hello server.");

        Ok::<_, Error>(())
    });
    try_join!(client, server)?;
    Ok(())
}
```


# 基准测试

运行速度: `raw` > `compress` > `compress_encrypt` > `encrypt`

但是目前这个基准测试并不严格，欢迎任何贡献！


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
