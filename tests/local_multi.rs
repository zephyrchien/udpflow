use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udpflow::{UdpSocket, UdpListener, UdpStreamLocal};

const BIND: &str = "127.0.0.1:10000";
const SENDER1: &str = "127.0.0.1:5000";
const SENDER2: &str = "127.0.0.1:6000";
const SENDER3: &str = "127.0.0.1:7000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn local_multi() {
    tokio::select! {
        _ = server() => {},
        _ = async {
            tokio::join!(
                async { tokio::join!(client(SENDER1, 0), client(SENDER2, 1)) },
                client(SENDER3, 2))
            } => {}
    };
}

async fn client(laddr: &'static str, idx: usize) {
    sleep(WAIT).await;

    let addr = BIND.parse::<SocketAddr>().unwrap();
    let socket = UdpSocket::bind(laddr).await.unwrap();
    let mut buf = [0u8; 32];

    for i in 0..5 {
        println!("client[{idx}]: send[{}]..", i);
        let n = socket.send_to(MSG, addr).await.unwrap();
        assert_eq!(n, MSG.len());

        println!("client[{idx}]: recv[{}]..", i);
        let (n, addr2) = socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(addr, addr2);
        assert_eq!(&buf[..n], MSG);
    }
}

async fn server() {
    let addr = BIND.parse::<SocketAddr>().unwrap();
    let listener = UdpListener::new(addr).unwrap();

    let mut idx = 0;

    loop {
        let mut buf = vec![0u8; 0x2000];
        let (n, stream, addr) = listener.accept(&mut buf).await.unwrap();
        buf.truncate(n);
        println!("server: handle {}", addr);
        tokio::spawn(handle(stream, idx, buf));
        idx += 1;
    }
}

async fn handle(mut stream: UdpStreamLocal, idx: usize, first_packet: Vec<u8>) {
    let mut buf = [0u8; 32];
    let mut i = 0;
    loop {
        println!("handle[{idx}]: recv[{}]..", i);
        let n = if i == 0 {
            let len = buf.len().min(first_packet.len());
            buf[..len].copy_from_slice(&first_packet[..len]);
            len
        } else {
            stream.read(&mut buf).await.unwrap()
        };
        assert_eq!(&buf[..n], MSG);

        println!("handle[{idx}]: send[{}]..", i);
        let n = stream.write(&buf[..n]).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }
}
