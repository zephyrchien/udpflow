use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::{net::UdpSocket, io::AsyncWriteExt};

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
    /// A listener must be continuously polled to recv packets or accept new streams.
    ///
    /// When receiving a packet from a known peer, this function does not return,
    /// and the packet will be copied then sent to the associated
    /// [`UdpStreamLocal`](super::UdpStreamLocal).  
    pub async fn accept(&self, buf: &mut [u8]) -> Result<(UdpStreamLocal, SocketAddr)> {
        let (n, addr) = self.socket.recv_from(buf).await?;
        debug_assert!(n != 0);

        let mut stream = UdpStreamLocal::new(self.socket.local_addr().unwrap(), addr).await?;

        stream.write_all(&buf[..n]).await?;

        Ok((stream, addr))
    }
}
