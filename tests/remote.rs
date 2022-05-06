use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udpflow::{UdpSocket, UdpStreamRemote};

const BIND: &str = "127.0.0.1:10000";
const SENDER: &str = "127.0.0.1:5000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn remote() {
    tokio::select! {
        _ = client() => {},
        _ = server() => {}
    };
}

async fn client() {
    sleep(WAIT).await;

    let addr = BIND.parse::<SocketAddr>().unwrap();
    let socket = UdpSocket::bind(SENDER).await.unwrap();
    let mut stream = UdpStreamRemote::new(socket, addr);
    let mut buf = [0u8; 32];

    for i in 0..5 {
        println!("client: send[{}]..", i);
        let n = stream.write(MSG).await.unwrap();
        assert_eq!(n, MSG.len());

        println!("client: recv[{}]..", i);
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], MSG);
    }
}

async fn server() {
    let socket = UdpSocket::bind(BIND).await.unwrap();

    let mut buf = vec![0u8; 32];
    let mut i = 0;

    loop {
        println!("server: recv[{}]..", i);
        let (n, addr) = socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(addr, SENDER.parse().unwrap());
        assert_eq!(&buf[..n], MSG);

        println!("server: send[{}]..", i);
        let n = socket.send_to(&buf[..n], addr).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }
}
