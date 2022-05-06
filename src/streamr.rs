use std::io::Result;
use std::net::SocketAddr;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::UdpSocket;
use tokio::time::{sleep, Sleep, Instant};
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

use crate::get_timeout;

/// Udp stream which is actively established.
///
/// A `Read` call times out when there is no packet received
/// during a period of time. This is treated as `EOF`, and
/// a `Ok(0)` will be returned.
pub struct UdpStreamRemote {
    socket: UdpSocket,
    timeout: Pin<Box<Sleep>>,
    addr: SocketAddr,
}

impl UdpStreamRemote {
    /// Create from a **bound** udp socket.
    #[inline]
    pub fn new(socket: UdpSocket, addr: SocketAddr) -> Self {
        Self {
            socket,
            addr,
            timeout: Box::pin(sleep(get_timeout())),
        }
    }

    /// Get peer sockaddr.
    #[inline]
    pub const fn peer_addr(&self) -> SocketAddr { self.addr }

    /// Get local sockaddr.
    #[inline]
    pub fn local_addr(&self) -> SocketAddr { self.socket.local_addr().unwrap() }

    /// Get inner udp socket.
    #[inline]
    pub const fn inner_socket(&self) -> &UdpSocket { &self.socket }
}

impl AsyncRead for UdpStreamRemote {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let this = self.get_mut();

        if let Poll::Ready(x) = this.socket.poll_recv_from(cx, buf) {
            // reset timer
            this.timeout.as_mut().reset(Instant::now() + get_timeout());

            return Poll::Ready(x.map(|_| ()));
        }

        // EOF
        if this.timeout.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Ok(()));
        }

        Poll::Pending
    }
}

impl AsyncWrite for UdpStreamRemote {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let this = self.get_mut();
        this.socket.poll_send_to(cx, buf, this.addr)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}
