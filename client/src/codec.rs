use tokio_io::codec::{Encoder, Decoder};
use bytes::BytesMut;
use pubsub::message::{Message, MessageHeader, MessageBuilder};
use pubsub::parser::{parse, ParseResult};

use std::io;

// Previous idiotic design decisions make this contrieved
// and stupid part necessary
fn build_message(header: MessageHeader, payload: Option<Vec<u8>>) -> io::Result<Message> {
    let mut builder = MessageBuilder::new();
    builder.message_type(header.message_type)
        .event_name(header.event_name);

    if let Some(payload) = payload {
        builder.payload(payload);
    }
    let message = try!(builder.build()
                       .or(Err(io::Error::new(io::ErrorKind::Other,
                                              "invalid message"))));
    Ok(message)
}

pub struct PubsubCodec;

impl Decoder for PubsubCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        match parse(&buf) {
            ParseResult::Completed(header, consumed, payload_len) => {
                if header.message_type.expects_payload() {
                    if buf.len() >= consumed + payload_len {
                        buf.split_to(consumed);
                        let payload = buf.split_to(payload_len).to_vec();
                        Ok(Some(try!(build_message(header, Some(payload)))))
                    }
                    else {
                        Ok(None)
                    }
                }
                else {
                    buf.split_to(consumed);
                    Ok(Some(try!(build_message(header, None))))
                }
            },

            ParseResult::Incomplete => Ok(None),
            ParseResult::Error => Err(io::Error::new(io::ErrorKind::Other,
                                                     "invalid message"))
        }
    }
}

impl Encoder for PubsubCodec {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        buf.extend(msg.into_bytes());
        Ok(())
    }
}
