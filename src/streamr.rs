use std::io::Result;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::UdpSocket;
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

pub struct UdpStreamRemote {
    socket: UdpSocket,
    addr: SocketAddr,
}

impl UdpStreamRemote {
    #[inline]
    pub const fn new(socket: UdpSocket, addr: SocketAddr) -> Self { Self { socket, addr } }

    #[inline]
    pub const fn peer_addr(&self) -> SocketAddr { self.addr }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr { self.socket.local_addr().unwrap() }

    #[inline]
    pub const fn inner_socket(&self) -> &UdpSocket { &self.socket }
}

impl AsyncRead for UdpStreamRemote {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        self.get_mut()
            .socket
            .poll_recv_from(cx, buf)
            .map(|x| x.map(|_| ()))
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
