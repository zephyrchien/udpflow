use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use udpflow::{UdpSocket, UdpStreamRemote};

const BIND: &str = "127.0.0.1:10000";
const SENDER: &str = "127.0.0.1:5000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn remote_timeout() {
    udpflow::set_timeout(TIMEOUT);
    tokio::select! {
        _ = client() => {},
        _ = server() => {}
    };
}

async fn client() {
    sleep(WAIT).await;

    let local = SENDER.parse::<SocketAddr>().unwrap();
    let remote = BIND.parse::<SocketAddr>().unwrap();
    let mut stream = UdpStreamRemote::new(local, remote).await.unwrap();
    let mut buf = [0u8; 32];

    for i in 0..3 {
        println!("client: send[{}]..", i);
        let n = stream.write(MSG).await.unwrap();
        assert_eq!(n, MSG.len());

        println!("client: recv[{}]..", i);
        let n = stream.read(&mut buf).await.unwrap();
        if n == 0 {
            println!("client: recv[{}].. EOF", i);
            // subsequent read returns 0
            for _ in 0..3 {
                let n = stream.read(&mut buf).await.unwrap();
                assert!(n == 0);
            }
            return;
        }
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

        if i == 2 {
            break;
        }

        println!("server: send[{}]..", i);
        let n = socket.send_to(&buf[..n], addr).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }

    println!("server: timeout..");
    sleep(TIMEOUT).await;
    println!("server: timeout elapsed..");
    sleep(TIMEOUT).await;
}
