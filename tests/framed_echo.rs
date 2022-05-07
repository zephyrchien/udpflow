use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, TcpListener};
use udpflow::UotStream;

const BIND: &str = "127.0.0.1:10000";
const MSG: &[u8] = b"Ciallo";
const WAIT: Duration = Duration::from_millis(500);

#[tokio::test]
async fn framed_echo() {
    tokio::select! {
        _ = client() => {},
        _ = server() => {}
    };
}

async fn client() {
    sleep(WAIT).await;

    let mut stream = UotStream::new(TcpStream::connect(BIND).await.unwrap());
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
    let listener = TcpListener::bind(BIND).await.unwrap();

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle(UotStream::new(stream)));
    }
}

async fn handle(mut stream: UotStream<TcpStream>) {
    let mut buf = [0u8; 32];
    let mut i = 0;
    loop {
        println!("server: recv[{}]..", i);
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], MSG);

        println!("server: send[{}]..", i);
        let n = stream.write(&buf[..n]).await.unwrap();
        assert_eq!(n, MSG.len());
        i += 1;
    }
}
