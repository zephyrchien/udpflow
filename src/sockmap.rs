use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use tokio::sync::mpsc::Sender;

pub(crate) type Packet = Vec<u8>;

#[derive(Clone)]
pub(crate) struct SockMap(Arc<RwLock<HashMap<SocketAddr, Sender<Packet>>>>);

impl SockMap {
    pub fn new() -> Self { Self(Arc::new(RwLock::new(HashMap::new()))) }

    #[inline]
    pub fn get(&self, addr: &SocketAddr) -> Option<Sender<Packet>> {
        // fetch the lock

        let sockmap = self.0.read().unwrap();

        sockmap.get(addr).cloned()

        // drop the lock
    }

    #[inline]
    pub fn insert(&self, addr: SocketAddr, tx: Sender<Packet>) {
        // fetch the lock
        let mut sockmap = self.0.write().unwrap();

        let _ = sockmap.insert(addr, tx);

        // drop the lock
    }

    #[inline]
    pub fn remove(&self, addr: &SocketAddr) {
        // fetch the lock
        let mut sockmap = self.0.write().unwrap();

        let _ = sockmap.remove(addr);

        // drop the lock
    }
}
