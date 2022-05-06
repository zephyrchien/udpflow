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

pub struct UotStream<T> {
    io: T,
    rd: State,
    wr: State,
}

impl<T> UotStream<T> {
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
                    let mut required = 2;

                    while required > 0 {
                        let n = ready!(Pin::new(&mut this.io).poll_read(cx, &mut len_be_buf))
                            .map(|_| len_be_buf.filled().len())?;
                        // EOF
                        if n == 0 {
                            this.rd = State::Fin;
                            return Poll::Ready(Ok(()));
                        }
                        required -= n;
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
                        iovec[0] = IoSlice::new(&len_be[..total]);
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
                            this.wr = State::Data(write_n as u16);
                        }
                        return Poll::Ready(Ok(write_n));
                    }
                }
            }
            State::Data(len) => {
                let to_write = len as usize;
                let n = ready!(Pin::new(&mut this.io).poll_write(cx, &buf[..to_write]))?;
                if n == 0 {
                    this.wr = State::Fin;
                    Poll::Ready(Ok(0))
                } else if n < to_write {
                    this.wr = State::Data((n - to_write) as u16);
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
