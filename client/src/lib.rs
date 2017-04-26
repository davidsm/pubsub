extern crate pubsub;
extern crate tokio_core;
extern crate tokio_io;
extern crate futures;
extern crate bytes;

mod codec;
mod client;

use codec::PubsubCodec;
pub use client::PubsubClient;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
