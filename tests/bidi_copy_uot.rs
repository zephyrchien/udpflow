use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tokio::net::{TcpStream, TcpListener};
use udpflow::{UdpSocket, UdpListener, UdpStreamLocal, UdpStreamRemote, UotStream};

const RELAY1: &str = "127.0.0.1:10000";
const RELAY2: &str = "127.0.0.1:15000";
const SENDER: &str = "127.0.0.1:5000";
const RECVER: &str = "127.0.0.1:20000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn bidi_copy_uot() {
    tokio::select! {
        _ = client() => {},
        _ = async {
            tokio::join!(
                tokio::spawn(relay_server1()),
                tokio::spawn(relay_server2()),
                tokio::spawn(echo_server())
            )
        } => {}
    };
}

async fn client() {
    sleep(WAIT).await;

    let addr = RELAY1.parse::<SocketAddr>().unwrap();
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

// udp -> tcp
async fn relay_server1() {
    let addr = RELAY1.parse::<SocketAddr>().unwrap();
    let listener = UdpListener::new(addr).unwrap();

    let mut buf = vec![0u8; 0x2000];

    loop {
        let (stream, addr) = listener.accept(&mut buf).await.unwrap();
        assert_eq!(addr, SENDER.parse().unwrap());
        tokio::spawn(handle1(stream));
    }
}

// recv packet, send framed data
async fn handle1(mut stream1: UdpStreamLocal) {
    let mut stream2 = UotStream::new(TcpStream::connect(RELAY2).await.unwrap());
    let _ = tokio::io::copy_bidirectional(&mut stream1, &mut stream2).await;
}

// tcp -> udp
async fn relay_server2() {
    let listener = TcpListener::bind(RELAY2).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        tokio::spawn(handle2(UotStream::new(stream)));
    }
}

// recv framed data, send packet
async fn handle2(mut stream1: UotStream<TcpStream>) {
    let local = "0.0.0.0:0".parse::<SocketAddr>().unwrap();
    let remote = RECVER.parse::<SocketAddr>().unwrap();
    let mut stream2 = UdpStreamRemote::new(local, remote).await.unwrap();
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
