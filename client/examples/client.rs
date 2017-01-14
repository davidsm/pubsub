extern crate pubsub_client;
extern crate tokio_core;
extern crate pubsub;
extern crate futures;

use pubsub::message::{Message, MessageBuilder, MessageType};
use pubsub_client::PubsubCodec;
use futures::{stream, Future, Sink};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_core::io::Io;
use std::io;

fn main() {
    let mut core = Core::new().unwrap();
    let remote_addr = "127.0.0.1:9876".parse().unwrap();

    let handle = core.handle();

    let work = TcpStream::connect(&remote_addr, &handle)
        .and_then(|socket| {
            // Once the socket has been established, use the `framed` helper to
            // create a transport.
            let transport = socket.framed(PubsubCodec);

            let mut builder = MessageBuilder::new();
            builder.message_type(MessageType::Subscribe)
                .event_name("foobar".to_string());
            let msg_1 = builder.build().unwrap();

            let mut builder = MessageBuilder::new();
            builder.message_type(MessageType::Subscribe)
                .event_name("blabla".to_string());

            let msg_2 = builder.build().unwrap();

            let messages: Vec<Result<Message, io::Error>> = vec![
                Ok(msg_1),
                Ok(msg_2)
            ];

            // Send all the messages to the remote. The strings will be encoded by
            // the `Codec`. `send_all` returns a future that completes once
            // everything has been sent.
            transport.send_all(stream::iter(messages))
        });

    core.run(work).unwrap();
}
