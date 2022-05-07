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

use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};

macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            Poll::Ready(t) => t,
            Poll::Pending => return Poll::Pending,
        }
    };
}

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
    io: T,
    rd: State,
    wr: State,
}

impl<T> UotStream<T> {
    /// Create from underlying IO source.
    #[inline]
    pub const fn new(io: T) -> Self {
        Self {
            io,
            rd: State::new(),
            wr: State::new(),
        }
    }
}

impl<T> AsRef<T> for UotStream<T> {
    #[inline]
    fn as_ref(&self) -> &T { &self.io }
}

impl<T> AsMut<T> for UotStream<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T { &mut self.io }
}

impl<T> AsyncRead for UotStream<T>
where
    T: AsyncRead + Unpin,
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
                    let mut len_be = [0u8; 2];
                    let mut len_be_buf = ReadBuf::new(&mut len_be);
                    let mut total = 0;

                    while total < 2 {
                        let n = ready!(Pin::new(&mut this.io).poll_read(cx, &mut len_be_buf))
                            .map(|_| len_be_buf.filled().len() - total)?;
                        // EOF
                        if n == 0 {
                            this.rd = State::Fin;
                            return Poll::Ready(Ok(()));
                        }
                        total += n;
                    }

                    this.rd = State::Data(u16::from_be_bytes(len_be));
                }
                State::Data(len) => {
                    debug_assert!(len as usize <= buf.remaining());
                    let mut buf_limit = ReadBuf::new(buf.initialize_unfilled_to(len as usize));
                    let n = ready!(Pin::new(&mut this.io).poll_read(cx, &mut buf_limit))
                        .map(|_| buf_limit.filled().len())?;

                    buf.advance(n);
                    if n == 0 {
                        this.rd = State::Fin;
                        return Poll::Ready(Ok(()));
                    } else if n as u16 == len {
                        this.rd = State::Len;
                        return Poll::Ready(Ok(()));
                    } else {
                        this.rd = State::Data(len - n as u16);
                    };
                }
                State::Fin => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl<T> AsyncWrite for UotStream<T>
where
    T: AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        debug_assert!(buf.len() < 0xffff);

        let this = self.get_mut();

        match this.wr {
            State::Len => {
                let len_be = (buf.len() as u16).to_be_bytes();
                let mut total = 0;
                let mut iovec = &mut [IoSlice::new(&len_be), IoSlice::new(buf)][..];
                loop {
                    let n = ready!(Pin::new(&mut this.io).poll_write_vectored(cx, iovec))?;
                    total += n;
                    // write zero
                    if n == 0 {
                        this.wr = State::Fin;
                        return Poll::Ready(Ok(0));
                    }
                    // write partial len
                    if total < 2 {
                        iovec[0] = IoSlice::new(&len_be[total..]);
                        continue;
                    } else if total == 2 {
                        iovec = &mut iovec[1..];
                        continue;
                    } else {
                        // write len + data
                        let write_n = total - 2;
                        if write_n == buf.len() {
                            this.wr = State::Len;
                        } else {
                            this.wr = State::Data((buf.len() - write_n) as u16);
                        }
                        return Poll::Ready(Ok(write_n));
                    }
                }
            }
            State::Data(len) => {
                let n = ready!(Pin::new(&mut this.io).poll_write(cx, &buf[..len as usize]))?;
                if n == 0 {
                    this.wr = State::Fin;
                    Poll::Ready(Ok(0))
                } else if n < len as usize {
                    this.wr = State::Data(len - n as u16);
                    Poll::Ready(Ok(n))
                } else {
                    this.wr = State::Len;
                    Poll::Ready(Ok(n))
                }
            }
            State::Fin => Poll::Ready(Ok(0)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
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
            let mut buf: Vec<u8> = Vec::with_capacity(65536);
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
                buf: Vec::with_capacity(65535),
                rlimit: 0,
                wlimit: wlimit,
                cursor: 0,
            });
            for i in 1..=512 {
                let prev = stream.io.buf.len();
                stream.write_all(&dummy[..i]).await.unwrap();
                let next = stream.io.buf.len();
                let buf = &stream.io.buf;
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
