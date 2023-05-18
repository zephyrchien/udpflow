//! UDP frame.
//!
//! ## Protocol Specification
//!
//! ```text
//! +------+----------+
//! | LEN  |   DATA   |
//! +------+----------+
//! |  2   | Variable |
//! +------+----------+
//! ```
//! LEN is a 16-bit unsigned integer in big endian byte order.
//!

use std::io::Result;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::BufReader;

use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

const MAX_DATAGRAM_PAYLOAD: usize = 65507;
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            Poll::Ready(t) => t,
            Poll::Pending => return Poll::Pending,
        }
    };
}

#[derive(Debug)]
enum State {
    Len,
    Data(u16),
    Fin,
}

impl State {
    #[inline]
    pub const fn new() -> Self { State::Len }
}

/// Framed UoT stream.
///
/// This is a simple wrapper over the underlying IO source.
///
/// A `Read` call always waits for a complete frame, and then decapsulate it.
///
/// A `Write` call will encapsulate the buffer in a frame. It only ensures `LEN` is sent,
/// the left `DATA` may be partially sent, which requires subsequent `Write` calls
/// to finish sending the entire frame. Usually this could be achieved by `write_all`.
pub struct UotStream<T> {
    rd: State,
    wr: State,
    buf: BufReader<T>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> UotStream<T> {
    /// Create from underlying IO source.
    #[inline]
    pub fn new(io: T) -> Self {
        Self {
            rd: State::new(),
            wr: State::Data(0),
            buf: BufReader::new(io),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsRef<T> for UotStream<T> {
    #[inline]
    fn as_ref(&self) -> &T { self.buf.get_ref() }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsMut<T> for UotStream<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T { self.buf.get_mut() }
}

impl<T> AsyncRead for UotStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let this = self.get_mut();

        loop {
            match this.rd {
                State::Len => {
                    let mut read_buf =
                        ReadBuf::new(buf.initialize_unfilled_to(2 - buf.filled().len()));
                    let n = ready!(Pin::new(&mut this.buf).poll_read(cx, &mut read_buf))
                        .map(|_| read_buf.filled().len())?;
                    if n == 0 {
                        this.rd = State::Fin;
                        buf.clear();
                        return Poll::Ready(Ok(()));
                    }
                    buf.advance(n);
                    if buf.filled().len() < 2 {
                        continue;
                    }
                    this.rd = State::Data(u16::from_be_bytes(buf.filled().try_into().unwrap()));
                    buf.clear();
                }
                State::Data(length) => {
                    let mut read_buf = ReadBuf::new(buf.initialize_unfilled_to(length as usize));
                    let n = ready!(Pin::new(&mut this.buf).poll_read(cx, &mut read_buf))
                        .map(|_| read_buf.filled().len())?;
                    if n == 0 {
                        this.rd = State::Fin;
                        buf.clear();
                        return Poll::Ready(Ok(()));
                    }
                    buf.advance(n);
                    if n != length as usize {
                        this.rd = State::Data(length - n as u16);
                        continue;
                    }
                    this.rd = State::Len;
                    return Poll::Ready(Ok(()));
                }
                State::Fin => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl<T> AsyncWrite for UotStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        assert!(buf.len() <= MAX_DATAGRAM_PAYLOAD);

        let this = self.get_mut();

        loop {
            match this.wr {
                State::Len => {
                    unreachable!();
                }
                State::Data(cursor) => {
                    let (n, written_data) = if cursor < 2 {
                        let len_be = &(buf.len() as u16).to_be_bytes()[cursor as usize..];
                        let iovec = &mut [IoSlice::new(len_be), IoSlice::new(buf)][..];
                        let n = ready!(Pin::new(&mut this.buf).poll_write_vectored(cx, iovec))?;
                        if n as u16 + cursor < 2 {
                            (n, 0)
                        } else {
                            (n, cursor as usize + n - 2)
                        }
                    } else {
                        let n = ready!(Pin::new(&mut this.buf).poll_write(cx, buf))?;
                        (n, n)
                    };

                    if n == 0 {
                        // EOF
                        this.wr = State::Fin;
                        return Poll::Ready(Ok(0));
                    }

                    this.wr = State::Data(cursor + n as u16);
                    if written_data > 0 {
                        if written_data == buf.len() {
                            this.wr = State::Data(0);
                        }
                        return Poll::Ready(Ok(written_data));
                    }
                }
                State::Fin => return Poll::Ready(Ok(0)),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().buf).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().buf).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct SlowStream {
        buf: Vec<u8>,
        rlimit: usize,
        wlimit: usize,
        cursor: usize,
    }

    impl AsyncRead for SlowStream {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let this = self.get_mut();
            let to_read = std::cmp::min(buf.remaining(), this.rlimit);
            let left_data = this.buf.len() - this.cursor;
            if left_data == 0 {
                return Poll::Ready(Ok(()));
            }
            if left_data <= to_read {
                buf.put_slice(&this.buf[this.cursor..]);
                this.cursor = this.buf.len();
                return Poll::Ready(Ok(()));
            }

            buf.put_slice(&this.buf[this.cursor..this.cursor + to_read]);
            this.cursor += to_read;
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for SlowStream {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            let this = self.get_mut();
            let len = std::cmp::min(buf.len(), this.wlimit);
            <_ as Write>::write(&mut this.buf, &buf[..len]).unwrap();
            Poll::Ready(Ok(len))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn read_framed() {
        async fn limit_at(rlimit: usize) {
            let dummy = vec![b'r'; 1024];
            let mut buf: Vec<u8> = Vec::with_capacity(MAX_DATAGRAM_PAYLOAD);
            for i in 1..=512 {
                <_ as Write>::write(&mut buf, &(i as u16).to_be_bytes()).unwrap();
                <_ as Write>::write(&mut buf, &dummy[..i]).unwrap();
            }
            let mut stream = UotStream::new(SlowStream {
                buf,
                rlimit,
                wlimit: 0,
                cursor: 0,
            });
            let mut buf = vec![0u8; 4096];
            for i in 1..=512 {
                let n = stream.read(&mut buf).await.unwrap();
                assert_eq!(n, i);
                assert_eq!(&buf[..n], &dummy[..n]);
            }
        }
        for i in 1..=512 {
            limit_at(i).await;
        }
    }

    #[tokio::test]
    async fn write_framed() {
        async fn limit_at(wlimit: usize) {
            let dummy = vec![b'w'; 1024];
            let mut stream = UotStream::new(SlowStream {
                buf: Vec::with_capacity(MAX_DATAGRAM_PAYLOAD),
                rlimit: 0,
                wlimit,
                cursor: 0,
            });
            for i in 1..=512 {
                let prev = stream.buf.get_ref().buf.len();
                stream.write_all(&dummy[..i]).await.unwrap();
                let next = stream.buf.get_ref().buf.len();
                let buf = &stream.buf.get_ref().buf;
                assert_eq!(next - prev, i + 2);
                assert_eq!(u16::from_be_bytes([buf[prev], buf[prev + 1]]), i as u16);
                assert_eq!(&buf[prev + 2..next], &dummy[..i]);
            }
        }
        for i in 1..=512 {
            limit_at(i).await;
        }
    }
}
