extern crate pubsub;
extern crate tokio_core;
extern crate futures;

mod codec;

pub use codec::PubsubCodec;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
