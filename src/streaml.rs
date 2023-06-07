use std::io::Result;

use std::net::{SocketAddr};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::UdpSocket;
use tokio::time::{sleep, Sleep, Instant};
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

use crate::{get_timeout, new_udp_socket};

/// Udp stream accepted from local listener.
///
/// A `Read` call times out when there is no packet received
/// during a period of time. This is treated as `EOF`, and
/// a `Ok(0)` will be returned.
pub struct UdpStreamLocal {
    socket: UdpSocket,
    timeout: Pin<Box<Sleep>>,
}

impl UdpStreamLocal {
    /// Create from a **bound** udp socket.
    #[inline]
    pub(crate) async fn new(
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> std::io::Result<Self> {
        let socket = new_udp_socket(local_addr)?;
        socket.connect(remote_addr).await?;
        Ok(Self {
            socket,
            timeout: Box::pin(sleep(get_timeout())),
        })
    }

    /// Get peer sockaddr.
    #[inline]
    pub fn peer_addr(&self) -> SocketAddr { self.socket.peer_addr().unwrap() }

    /// Get local sockaddr.
    #[inline]
    pub fn local_addr(&self) -> SocketAddr { self.socket.local_addr().unwrap() }

    /// Get inner udp socket.
    #[inline]
    pub const fn inner_socket(&self) -> &UdpSocket { &self.socket }
}

impl AsyncRead for UdpStreamLocal {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let this = self.get_mut();

        if let Poll::Ready(result) = this.socket.poll_recv(cx, buf) {
            // reset timer
            this.timeout.as_mut().reset(Instant::now() + get_timeout());

            return match result {
                Ok(_) => Poll::Ready(Ok(())),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
                Err(e) => Poll::Ready(Err(e)),
            };
        }

        // EOF
        if this.timeout.as_mut().poll(cx).is_ready() {
            buf.clear();
            return Poll::Ready(Ok(()));
        }

        Poll::Pending
    }
}

impl AsyncWrite for UdpStreamLocal {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let this = self.get_mut();
        this.socket.poll_send(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}
