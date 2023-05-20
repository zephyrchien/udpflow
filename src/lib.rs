//! Stream API for tokio-udp.
//!
//! ## TCP-like UDP stream
//!
//! ```
//! use tokio::net::UdpSocket;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//! use udpflow::{UdpListener, UdpStreamLocal, UdpStreamRemote};
//! async fn server() {
//!     let addr = "127.0.0.1:5000".parse().unwrap();
//!     let listener = UdpListener::new(addr).unwrap();
//!     let mut buf = vec![0u8; 0x2000];
//!     // listener must be continuously polled to recv packets or accept new streams
//!     while let Ok((stream, addr)) = listener.accept(&mut buf).await {
//!         tokio::spawn(handle(stream));
//!     }
//! }
//!
//! async fn handle(mut stream1: UdpStreamLocal) {
//!     let local = "127.0.0.1:0".parse().unwrap();
//!     let remote = "127.0.0.1:10000".parse().unwrap();
//!     let mut stream2 = UdpStreamRemote::new(local, remote).await.unwrap();
//!     let mut buf = vec![0u8; 256];
//!     stream1.read(&mut buf).await; stream2.write(&buf).await;
//!     stream2.read(&mut buf).await; stream1.write(&buf).await;
//! }
//! ```
//!
//! ## Send/Recv framed data
//!
//! ```
//! use tokio::net::TcpStream;
//! use udpflow::UotStream;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//! async {
//!     let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
//!     let mut stream = UotStream::new(stream);
//!     let mut buf = vec![0u8; 256];
//!     // read a frame
//!     let n = stream.read(&mut buf).await.unwrap();
//!     // write a frame
//!     stream.write_all(&buf[..n]).await;
//! };
//! ```
//!

mod streaml;
mod streamr;
mod listener;

pub mod frame;

pub use listener::UdpListener;
pub use streaml::UdpStreamLocal;
pub use streamr::UdpStreamRemote;
pub use frame::UotStream;

/// Re-export from tokio-udp.
pub use tokio::net::UdpSocket;

mod statics {
    use std::time::Duration;

    static mut TIMEOUT: Duration = Duration::from_secs(20);

    /// Get read timeout.
    pub fn get_timeout() -> Duration { unsafe { TIMEOUT } }

    /// Set read timeout.
    pub fn set_timeout(timeout: Duration) {
        unsafe {
            TIMEOUT = timeout;
        }
    }
}

pub use statics::{set_timeout, get_timeout};

pub(crate) fn new_udp_socket(local_addr: std::net::SocketAddr) -> std::io::Result<UdpSocket> {
    let udp_sock = socket2::Socket::new(
        if local_addr.is_ipv4() {
            socket2::Domain::IPV4
        } else {
            socket2::Domain::IPV6
        },
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    udp_sock.set_reuse_address(true)?;
    #[cfg(not(windows))]
    udp_sock.set_reuse_port(true)?;
    udp_sock.set_nonblocking(true)?;
    udp_sock.bind(&socket2::SockAddr::from(local_addr))?;
    let udp_sock: std::net::UdpSocket = udp_sock.into();
    udp_sock.try_into()
}
