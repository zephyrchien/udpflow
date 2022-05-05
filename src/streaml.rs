use std::io::Result;
use std::sync::Arc;
use std::net::SocketAddr;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::UdpSocket;
use tokio::time::{sleep, Sleep, Instant};
use tokio::sync::mpsc::Receiver;
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

use crate::sockmap::{SockMap, Packet};
use crate::get_timeout;

pub struct UdpStreamLocal {
    rx: Receiver<Packet>,
    socket: Arc<UdpSocket>,
    timeout: Pin<Box<Sleep>>,
    sockmap: SockMap,
    addr: SocketAddr,
}

impl UdpStreamLocal {
    pub(crate) fn new(
        rx: Receiver<Packet>,
        socket: Arc<UdpSocket>,
        sockmap: SockMap,
        addr: SocketAddr,
    ) -> Self {
        Self {
            rx,
            socket,
            addr,
            sockmap,
            timeout: Box::pin(sleep(get_timeout())),
        }
    }

    #[inline]
    pub const fn peer_addr(&self) -> SocketAddr { self.addr }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr { self.socket.local_addr().unwrap() }

    #[inline]
    pub const fn inner_socket(&self) -> &Arc<UdpSocket> { &self.socket }
}

impl Drop for UdpStreamLocal {
    fn drop(&mut self) { self.sockmap.remove(&self.addr); }
}

impl AsyncRead for UdpStreamLocal {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let this = self.get_mut();

        if let Poll::Ready(Some(pkt)) = this.rx.poll_recv(cx) {
            buf.put_slice(&pkt);

            // reset timer
            this.timeout.as_mut().reset(Instant::now() + get_timeout());

            return Poll::Ready(Ok(()));
        }

        // EOF
        if this.timeout.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Ok(()));
        }

        Poll::Pending
    }
}

impl AsyncWrite for UdpStreamLocal {
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
