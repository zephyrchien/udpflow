use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::UdpStreamLocal;

use crate::sockmap::{SockMap, Packet};

pub struct UdpListener {
    socket: Arc<UdpSocket>,
    sockmap: SockMap,
}

impl UdpListener {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket: Arc::new(socket),
            sockmap: SockMap::new(),
        }
    }

    pub async fn accept(&mut self, buf: &mut [u8]) -> Result<(UdpStreamLocal, SocketAddr)> {
        loop {
            let (n, addr) = self.socket.recv_from(buf).await?;
            debug_assert!(n != 0);

            // existed session
            if let Some(tx) = self.sockmap.get(&addr) {
                let _ = tx.send(Vec::from(&buf[..n])).await;
                continue;
            }

            // new session
            let (tx, rx) = mpsc::channel::<Packet>(4);
            self.sockmap.insert(addr, tx);

            let stream = UdpStreamLocal::new(rx, self.socket.clone(), self.sockmap.clone(), addr);
            return Ok((stream, addr));
        }
    }
}
