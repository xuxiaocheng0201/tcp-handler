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

fn protocols(c: &mut Criterion) {
    c.bench_function("create", |b| b.iter(|| { create(); }));

    c.bench_function("create raw", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_raw()));
    c.bench_function("create compress", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_compress()));
    c.bench_function("create encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_encrypt()));
    c.bench_function("create compress_encrypt", |b| b.to_async(Runtime::new().unwrap()).iter(|| create_compress_encrypt()));


    async fn _send_recv<S: TcpHandler, R: TcpHandler>(sender: &mut S, receiver: &mut R) -> Result<()> {
        let bytes = BytesMut::zeroed(1 << 20);
        sender.handler_send(&mut bytes.freeze()).await?;
        receiver.handler_recv().await?;
        Ok(())
    }
}

criterion_group!(benches, protocols);
criterion_main!(benches);
