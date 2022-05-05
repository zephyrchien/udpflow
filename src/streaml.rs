use std::io::Result;
use std::sync::Arc;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

use crate::Packet;

pub struct UdpStreamL {
    rx: Receiver<Packet>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
}

impl UdpStreamL {
    pub(crate) fn new(rx: Receiver<Packet>, socket: Arc<UdpSocket>, addr: SocketAddr) -> Self {
        Self { rx, socket, addr }
    }

    #[inline]
    pub const fn peer_addr(&self) -> SocketAddr { self.addr }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr { self.socket.local_addr().unwrap() }

    #[inline]
    pub const fn inner_socket(&self) -> &Arc<UdpSocket> { &self.socket }
}

impl AsyncRead for UdpStreamL {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        use Poll::*;

        match self.get_mut().rx.poll_recv(cx) {
            Ready(Some(pkt)) => {
                buf.put_slice(&pkt);
                Ready(Ok(()))
            }
            Ready(None) => Ready(Ok(())),
            Pending => Pending,
        }
    }
}

impl AsyncWrite for UdpStreamL {
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
