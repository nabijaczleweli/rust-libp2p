// Copyright 2020 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::{
    io,
    os::unix::net,
    task::{Context, Poll},
};

use async_io::Async;
use futures::future::{BoxFuture, FutureExt};

use super::{Incoming, Provider};

/// A UNIX-domain stream [`Transport`](libp2p_core::Transport) that works with the `async-std`
/// ecosystem.
///
/// # Example
///
/// ```rust
/// # use libp2p_unix_stream as unix_stream;
/// # use libp2p_core::{Transport, transport::ListenerId};
/// # use futures::future;
/// # use std::pin::Pin;
/// #
/// # #[async_std::main]
/// # async fn main() {
/// let mut transport = unix_stream::async_io::Transport::new();
/// let id = ListenerId::next();
/// let _ = std::fs::remove_file("socket");
/// transport
///     .listen_on(id, "/unix/socket".parse().unwrap())
///     .unwrap();
///
/// let addr = future::poll_fn(|cx| Pin::new(&mut transport).poll(cx))
///     .await
///     .into_new_address()
///     .unwrap();
///
/// println!("Listening on {addr}");
/// # let _ = std::fs::remove_file("socket");
/// # }
/// ```
pub type Transport = crate::Transport<UnixStrm>;

#[derive(Copy, Clone)]
#[doc(hidden)]
pub enum UnixStrm {}

impl Provider for UnixStrm {
    type Stream = UnixStream;
    type Listener = Async<net::UnixListener>;

    fn new_listener(l: net::UnixListener) -> io::Result<Self::Listener> {
        Async::new(l)
    }

    fn new_stream(s: net::UnixStream) -> BoxFuture<'static, io::Result<Self::Stream>> {
        async move {
            // Taken from [`Async::connect`].

            let stream = Async::new(s)?;

            // The stream becomes writable when connected.
            stream.writable().await?;

            // Check if there was an error while connecting.
            match stream.get_ref().take_error()? {
                None => Ok(stream),
                Some(err) => Err(err),
            }
        }
        .boxed()
    }

    fn poll_accept(
        l: &mut Self::Listener,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Incoming<Self::Stream>>> {
        let (stream, remote_addr) = loop {
            match l.poll_readable(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Ready(Ok(())) => match l.accept().now_or_never() {
                    Some(Err(e)) => return Poll::Ready(Err(e)),
                    Some(Ok(res)) => break res,
                    None => {
                        // Since it doesn't do any harm, account for false positives of
                        // `poll_readable` just in case, i.e. try again.
                    }
                },
            }
        };

        let local_addr = stream.get_ref().local_addr()?;

        Poll::Ready(Ok(Incoming {
            stream,
            local_addr,
            remote_addr,
        }))
    }
}

pub type UnixStream = Async<net::UnixStream>;
