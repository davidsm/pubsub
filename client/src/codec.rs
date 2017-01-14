use tokio_core::io::{Codec, EasyBuf};
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

impl Codec for PubsubCodec {
    type In = Message;
    type Out = Message;

    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Self::In>> {
        match parse(buf.as_slice()) {
            ParseResult::Completed(header, consumed, payload_len) => {
                if header.message_type.expects_payload() {
                    if buf.len() >= consumed + payload_len {
                        buf.drain_to(consumed);
                        let payload = buf.drain_to(payload_len).as_slice().to_vec();
                        Ok(Some(try!(build_message(header, Some(payload)))))
                    }
                    else {
                        Ok(None)
                    }
                }
                else {
                    buf.drain_to(consumed);
                    Ok(Some(try!(build_message(header, None))))
                }
            },

            ParseResult::Incomplete => Ok(None),
            ParseResult::Error => Err(io::Error::new(io::ErrorKind::Other,
                                                     "invalid message"))
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        Ok(buf.extend(msg.to_bytes()))
    }
}
