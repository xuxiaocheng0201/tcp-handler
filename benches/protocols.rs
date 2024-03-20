use std::future::Future;
use anyhow::Result;
use bytes::BytesMut;
use criterion::{Criterion, criterion_group, criterion_main};
use tokio::io::{AsyncRead, AsyncWrite, duplex, split};
use tokio::runtime::Runtime;
use tokio::{spawn, try_join};
use tcp_handler::compress::{TcpClientHandlerCompress, TcpServerHandlerCompress};
use tcp_handler::compress_encrypt::{TcpClientHandlerCompressEncrypt, TcpServerHandlerCompressEncrypt};
use tcp_handler::encrypt::{TcpClientHandlerEncrypt, TcpServerHandlerEncrypt};
use tcp_handler::raw::{TcpClientHandlerRaw, TcpServerHandlerRaw};
use tcp_handler::TcpHandler;

fn create() -> (impl AsyncRead + Unpin, impl AsyncWrite + Unpin, impl AsyncRead + Unpin, impl AsyncWrite + Unpin) {
    let (client, server) = duplex(1024);
    let (cr, cw) = split(client);
    let (sr, sw) = split(server);
    (cr, cw, sr, sw)
}
async fn create_raw() -> Result<(impl TcpHandler, impl TcpHandler)> {
    let (cr, cw, sr, sw) = create();
    let server = spawn(TcpServerHandlerRaw::new(sr, sw, "test", |v| v == "0", "0"));
    let client = spawn(TcpClientHandlerRaw::new(cr, cw, "test", "0"));
    let (server, client) = try_join!(server, client)?;
    Ok((server?, client?))
}
async fn create_compress() -> Result<(impl TcpHandler, impl TcpHandler)> {
    let (cr, cw, sr, sw) = create();
    let server = spawn(TcpServerHandlerCompress::new(sr, sw, "test", |v| v == "0", "0"));
    let client = spawn(TcpClientHandlerCompress::new(cr, cw, "test", "0"));
    let (server, client) = try_join!(server, client)?;
    Ok((server?, client?))
}
async fn create_encrypt() -> Result<(impl TcpHandler, impl TcpHandler)> {
    let (cr, cw, sr, sw) = create();
    let server = spawn(TcpServerHandlerEncrypt::new(sr, sw, "test", |v| v == "0", "0"));
    let client = spawn(TcpClientHandlerEncrypt::new(cr, cw, "test", "0"));
    let (server, client) = try_join!(server, client)?;
    Ok((server?, client?))
}
async fn create_compress_encrypt() -> Result<(impl TcpHandler, impl TcpHandler)> {
    let (cr, cw, sr, sw) = create();
    let server = spawn(TcpServerHandlerCompressEncrypt::new(sr, sw, "test", |v| v == "0", "0"));
    let client = spawn(TcpClientHandlerCompressEncrypt::new(cr, cw, "test", "0"));
    let (server, client) = try_join!(server, client)?;
    Ok((server?, client?))
}

async fn send_recv<F: Future<Output = Result<(impl TcpHandler, impl TcpHandler)>>>(future: F) -> Result<()> {
    let (mut sender, mut receiver) = future.await?;
    let bytes = BytesMut::zeroed(1 << 20);
    sender.handler_send(&mut bytes.freeze()).await?;
    receiver.handler_recv().await?;
    Ok(())
}

fn protocols(c: &mut Criterion) {
    c.bench_function("create", |b| b.iter(|| { create(); }));

    c.bench_function("create raw", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_raw()));
    c.bench_function("create compress", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_compress()));
    c.bench_function("create encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_encrypt()));
    c.bench_function("create compress_encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_compress_encrypt()));

    c.bench_function("raw", |b| b.to_async(Runtime::new().unwrap()).iter(|| send_recv(create_raw())));
    c.bench_function("compress", |b| b.to_async(Runtime::new().unwrap()).iter(|| send_recv(create_compress())));
    c.bench_function("encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| send_recv(create_encrypt())));
    c.bench_function("compress_encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| send_recv(create_compress_encrypt())));
}

criterion_group!(name = benches; config = Criterion::default().sample_size(20); targets = protocols);
criterion_main!(benches);

/*
create                  time:   [635.03 ns 637.77 ns 640.25 ns]

create raw              time:   [67.539 µs 68.330 µs 68.976 µs]
Found 2 outliers among 20 measurements (10.00%)
  2 (10.00%) high severe

create compress         time:   [67.276 µs 67.756 µs 68.279 µs]
Found 1 outliers among 20 measurements (5.00%)
  1 (5.00%) high mild

Benchmarking create encrypt: Warming up for 3.0000 s
Warning: Unable to complete 20 samples in 5.0s. You may wish to increase target time to 24.1s, or reduce sample count to 10.
create encrypt          time:   [1.1634 s 1.4086 s 1.6721 s]

Benchmarking create compress_encrypt: Warming up for 3.0000 s
Warning: Unable to complete 20 samples in 5.0s. You may wish to increase target time to 20.3s, or reduce sample count to 10.
create compress_encrypt time:   [1.1133 s 1.4855 s 1.8974 s]
*/
