use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use udpflow::{UdpSocket, UdpListener, UdpStreamLocal, UdpStreamRemote};

const BIND: &str = "127.0.0.1:10000";
const SENDER: &str = "127.0.0.1:5000";
const RECVER: &str = "127.0.0.1:15000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn bidi_copy() {
    tokio::select! {
        _ = client() => {},
        _ = async { tokio::join!(echo_server(),relay_server()) } => {}
    };
}

async fn client() {
    sleep(WAIT).await;

    let addr = BIND.parse::<SocketAddr>().unwrap();
    let socket = UdpSocket::bind(SENDER).await.unwrap();
    let mut buf = [0u8; 32];

    for i in 0..5 {
        println!("client: send[{}]..", i);
        let n = socket.send_to(MSG, addr).await.unwrap();
        assert_eq!(n, MSG.len());

        println!("client: recv[{}]..", i);
        let (n, addr2) = socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(addr, addr2);
        assert_eq!(&buf[..n], MSG);
    }
}

async fn relay_server() {
    let socket = UdpSocket::bind(BIND).await.unwrap();
    let listener = UdpListener::new(socket);

    let mut buf = vec![0u8; 0x2000];

    while let Ok((stream, addr)) = listener.accept(&mut buf).await {
        assert_eq!(addr, SENDER.parse().unwrap());
        tokio::spawn(handle(stream));
    }
}

async fn handle(mut stream1: UdpStreamLocal) {
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let mut stream2 = UdpStreamRemote::new(socket, RECVER.parse().unwrap());
    let _ = tokio::io::copy_bidirectional(&mut stream1, &mut stream2).await;
}

async fn echo_server() {
    let socket = UdpSocket::bind(RECVER).await.unwrap();

    let mut buf = vec![0u8; 32];
    let mut i = 0;

    loop {
        println!("server: recv[{}]..", i);
        let (n, addr) = socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], MSG);

        println!("server: send[{}]..", i);
        let n = socket.send_to(&buf[..n], addr).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }
}
