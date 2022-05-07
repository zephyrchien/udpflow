# UDPflow

[![crates.io](https://img.shields.io/crates/v/udpflow.svg)](https://crates.io/crates/udpflow)
[![Released API docs](https://docs.rs/udpflow/badge.svg)](https://docs.rs/udpflow)

Stream API for tokio-udp.

## TCP-like UDP stream

```rust
use tokio::net::UdpSocket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udpflow::{UdpListener, UdpStreamLocal, UdpStreamRemote};
async fn server() {
    let socket = UdpSocket::bind("127.0.0.1:5000").await.unwrap();
    let listener = UdpListener::new(socket);
    let mut buf = vec![0u8; 0x2000];
    // listener must be continuously polled to recv packets or accept new streams
    while let Ok((stream, addr)) = listener.accept(&mut buf).await {
        tokio::spawn(handle(stream));
    }
}
async fn handle(mut stream1: UdpStreamLocal) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut stream2 = UdpStreamRemote::new(socket, "127.0.0.1:10000".parse().unwrap());
    let mut buf = vec![0u8; 256];
    stream1.read(&mut buf).await; stream2.write(&buf).await;
    stream2.read(&mut buf).await; stream1.write(&buf).await;
}
```

## Send/Recv framed data

```rust
use tokio::net::TcpStream;
use udpflow::UotStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async {
    let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut stream = UotStream::new(stream);
    let mut buf = vec![0u8; 256];
    // read a frame
    let n = stream.read(&mut buf).await.unwrap();
    // write a frame
    stream.write_all(&buf[..n]).await;
};
```

## UoT Specification

```plain
+------+----------+
| LEN  |   DATA   |
+------+----------+
|  2   | Variable |
+------+----------+
```

*LEN is a 16-bit unsigned integer in big endian byte order.
