use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::UdpStreamLocal;

use crate::sockmap::{SockMap, Packet};

/// Udp packet listener.
pub struct UdpListener {
    socket: Arc<UdpSocket>,
    sockmap: SockMap,
}

impl UdpListener {
    /// Create from a **bound** udp socket.
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket: Arc::new(socket),
            sockmap: SockMap::new(),
        }
    }

    /// Accept a new stream.
    ///
    /// When receiving a packet from a known peer, this function does not return,
    /// and the packet will be copied then sent to the associated
    /// [`UdpStreamLocal`](super::UdpStreamLocal).  
    pub async fn accept(&self, buf: &mut [u8]) -> Result<(UdpStreamLocal, SocketAddr)> {
        loop {
            let (n, addr) = self.socket.recv_from(buf).await?;
            debug_assert!(n != 0);

            // existed session
            if let Some(tx) = self.sockmap.get(&addr) {
                let _ = tx.send(Vec::from(&buf[..n])).await;
                continue;
            }

            // new session
            let (tx, rx) = mpsc::channel::<Packet>(32);
            let _ = tx.send(Vec::from(&buf[..n])).await;
            self.sockmap.insert(addr, tx);

            let stream = UdpStreamLocal::new(rx, self.socket.clone(), self.sockmap.clone(), addr);
            return Ok((stream, addr));
        }
    }
}
