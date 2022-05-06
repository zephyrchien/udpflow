mod sockmap;
mod streaml;
mod streamr;
mod listener;

pub mod frame;

pub use listener::UdpListener;
pub use streaml::UdpStreamLocal;
pub use streamr::UdpStreamRemote;
pub use frame::UotStream;

/// Re-export from tokio-udp. 
pub use tokio::net::UdpSocket;

mod statics {
    use std::time::Duration;

    static mut TIMEOUT: Duration = Duration::from_secs(20);

    /// Get read timeout.
    pub fn get_timeout() -> Duration { unsafe { TIMEOUT } }

    /// Set read timeout.
    pub fn set_timeout(timeout: Duration) {
        unsafe {
            TIMEOUT = timeout;
        }
    }
}

pub use statics::{set_timeout, get_timeout};
