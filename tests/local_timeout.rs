use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udpflow::{UdpSocket, UdpListener, UdpStreamLocal};

const BIND: &str = "127.0.0.1:10000";
const SENDER: &str = "127.0.0.1:5000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn local_timeout() {
    udpflow::set_timeout(TIMEOUT);
    tokio::select! {
        _ = client() => {},
        _ = server() => {}
    };
}

async fn client() {
    let addr = BIND.parse::<SocketAddr>().unwrap();
    let socket = UdpSocket::bind(SENDER).await.unwrap();
    let mut buf = [0u8; 32];

    for i in 0..3 {
        println!("client: wait[{}]..", i);
        sleep(WAIT).await;
        println!("client: send[{}]..", i);
        let n = socket.send_to(MSG, addr).await.unwrap();
        assert_eq!(n, MSG.len());

        println!("client: recv[{}]..", i);
        let (n, addr2) = socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(addr, addr2);
        assert_eq!(&buf[..n], MSG);
    }

    println!("client: timeout..");
    sleep(TIMEOUT).await;
    println!("client: timeout elapsed..");
    sleep(TIMEOUT).await;
}

async fn server() {
    let addr = BIND.parse::<SocketAddr>().unwrap();
    let listener = UdpListener::new(addr).unwrap();

    loop {
        let mut buf = vec![0u8; 0x2000];
        let (n, stream, addr) = listener.accept(&mut buf).await.unwrap();
        buf.truncate(n);
        assert_eq!(addr, SENDER.parse().unwrap());
        tokio::spawn(handle(stream, buf));
    }
}

async fn handle(mut stream: UdpStreamLocal, first_packet: Vec<u8>) {
    let mut buf = [0u8; 32];
    let mut i = 0;
    loop {
        println!("server: recv[{}]..", i);
        let n = if i == 0 {
            let len = buf.len().min(first_packet.len());
            buf[..len].copy_from_slice(&first_packet[..len]);
            len
        } else {
            stream.read(&mut buf).await.unwrap()
        };

        if n == 0 {
            println!("server: recv[{}].. EOF", i);
            // subsequent read returns 0
            for _ in 0..3 {
                let n = stream.read(&mut buf).await.unwrap();
                assert!(n == 0);
            }
            return;
        }

        assert_eq!(&buf[..n], MSG);

        println!("server: send[{}]..", i);
        let n = stream.write(&buf[..n]).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }
}
