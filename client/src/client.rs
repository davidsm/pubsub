use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::codec::Framed;
use tokio_io::{AsyncRead, AsyncWrite};
use futures::future::Future;

use PubsubCodec;

use std::io;
use std::net::SocketAddr;

type PubsubFuture = Box<Future<Item=Framed<TcpStream, PubsubCodec>, Error=io::Error>>;

pub struct PubsubClient;

impl PubsubClient {
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> PubsubFuture {
        Box::new(TcpStream::connect(addr, &handle)
            .and_then(|socket| {
                Ok(socket.framed(PubsubCodec))
            }))
    }
}
