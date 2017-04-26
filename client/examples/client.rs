extern crate pubsub_client;
extern crate tokio_core;
extern crate pubsub;
extern crate futures;

use pubsub::message::{Message, MessageBuilder, MessageType};
use pubsub_client::PubsubClient;
use futures::{Stream, Sink, Future};
use tokio_core::reactor::Core;


fn main() {
    let mut core = Core::new().unwrap();
    let remote_addr = "127.0.0.1:9876".parse().unwrap();

    let handle = core.handle();
    let client = PubsubClient::connect(&remote_addr, &handle)
        .and_then(|transport| {
            let mut builder = MessageBuilder::new();
            builder.message_type(MessageType::Subscribe)
                .event_name("foobar".to_string());
            let msg = builder.build().unwrap();
            transport.send(msg)
        })
        .and_then(|transport| {
            transport.for_each(|msg| {
                println!("{:?}", msg);
                Ok(())
            })
        });

    core.run(client).unwrap();
}
