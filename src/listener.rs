use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, Sender};

use crate::UdpStreamL;

pub(crate) type Packet = Vec<u8>;
pub(crate) type SockMap = HashMap<SocketAddr, Sender<Packet>>;

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

    pub async fn accept(&mut self, buf: &mut [u8]) -> Result<(UdpStreamL, SocketAddr)> {
        loop {
            let (n, addr) = self.socket.recv_from(buf).await?;
            debug_assert!(n != 0);

            // existed session
            if let Some(tx) = self.sockmap.get(&addr) {
                if let Err(e) = tx.send(Vec::from(&buf[..n])).await {
                    return Err(Error::new(ErrorKind::Other, e));
                }
                continue;
            }

            // new session
            let (tx, rx) = mpsc::channel::<Packet>(4);
            self.sockmap.insert(addr, tx);

            let stream = UdpStreamL::new(rx, self.socket.clone(), addr);
            return Ok((stream, addr));
        }
    }
}
