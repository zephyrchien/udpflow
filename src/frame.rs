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

use std::io::{IoSlice, Result};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::marker::PhantomData;

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

pub struct DirectRead;
pub struct DirectWrite;
pub struct FramedRead;
pub struct FramedWrite;

pub struct UotStream<T, R = DirectRead, W = DirectWrite> {
    io: T,
    rd: State,
    wr: State,
    __rd: PhantomData<R>,
    __wr: PhantomData<W>,
}

impl<T, R, W> UotStream<T, R, W> {
    #[inline]
    pub const fn new(io: T) -> Self {
        Self {
            io,
            rd: State::new(),
            wr: State::new(),
            __rd: PhantomData,
            __wr: PhantomData,
        }
    }

    #[inline]
    pub fn use_framed_read(self) -> UotStream<T, FramedRead, W> {
        UotStream {
            io: self.io,
            rd: self.rd,
            wr: self.wr,
            __rd: PhantomData,
            __wr: PhantomData,
        }
    }

    #[inline]
    pub fn use_framed_write(self) -> UotStream<T, R, FramedWrite> {
        UotStream {
            io: self.io,
            rd: self.rd,
            wr: self.wr,
            __rd: PhantomData,
            __wr: PhantomData,
        }
    }
}

impl<T, R, W> AsRef<T> for UotStream<T, R, W> {
    #[inline]
    fn as_ref(&self) -> &T { &self.io }
}

impl<T, R, W> AsMut<T> for UotStream<T, R, W> {
    #[inline]
    fn as_mut(&mut self) -> &mut T { &mut self.io }
}

// default impl

impl<T> AsyncRead for UotStream<T>
where
    T: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_read(cx, buf)
    }
}

impl<T> AsyncWrite for UotStream<T>
where
    T: AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
    }
}

// framed impl

impl<T, W> AsyncRead for UotStream<T, FramedRead, W>
where
    W: Unpin,
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
                    let n = ready!(Pin::new(&mut this.io).poll_read(cx, buf))
                        .map(|_| buf.filled().len())? as u16;

                    if n == 0 {
                        this.rd = State::Fin;
                    } else if n == len {
                        this.rd = State::Len;
                    } else {
                        this.rd = State::Data(len - n);
                    };

                    return Poll::Ready(Ok(()));
                }
                State::Fin => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl<T, R> AsyncWrite for UotStream<T, R, FramedWrite>
where
    R: Unpin,
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
