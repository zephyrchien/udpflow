use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::{UdpStreamLocal, new_udp_socket};

/// Udp packet listener.
pub struct UdpListener {
    socket: Arc<UdpSocket>,
}

impl UdpListener {
    /// Create from a **bound** udp socket.
    pub fn new(local_address: SocketAddr) -> std::io::Result<Self> {
        Ok(Self {
            socket: Arc::new(new_udp_socket(local_address)?),
        })
    }

    /// Accept a new stream.
    ///
    /// On success, it returns peer stream socket, peer address and
    /// the number of bytes read.
    /// [`UdpStreamLocal`](super::UdpStreamLocal).  
    pub async fn accept(&self, buf: &mut [u8]) -> Result<(usize, UdpStreamLocal, SocketAddr)> {
        let (n, addr) = self.socket.recv_from(buf).await?;

        debug_assert!(n != 0);

        let stream = UdpStreamLocal::new(self.socket.local_addr().unwrap(), addr).await?;

        Ok((n, stream, addr))
    }
}
