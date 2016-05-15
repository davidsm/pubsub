use mio;
use mio::tcp::{TcpStream};
use mio::{EventSet, PollOpt, TryRead};

use pubsub::message::MessageHeader;
use pubsub::parser::{parse, ParseResult};

use server::PubsubServer;

use std::{u8, u16};


const MAX_PACKET_SIZE: usize =
    1 // Type
    + 1 // Name length
    + u8::MAX as usize // Name
    + 2 // Payload length
    + u16::MAX as usize; // Payload

pub enum ClientAction {
    Subscribe(String),
    Publish(String, Vec<u8>),
    Unsubscribe(String),
    Error,
    Nothing
}

enum ReadState {
    Header(usize),
    Payload(MessageHeader, usize, usize)
}

struct BufferState {
    read_index: usize,
    write_index: usize
}

impl BufferState {
    fn reset(&mut self) {
        self.read_index = 0;
        self.write_index = 0;
    }
}

pub struct PubsubClient {
    socket: TcpStream,
    token: mio::Token,
    read_state: ReadState,
    buffer: [u8; MAX_PACKET_SIZE],
    buffer_state: BufferState
}

impl PubsubClient {
    pub fn new(socket: TcpStream, token: mio::Token) -> PubsubClient {
        PubsubClient {
            socket: socket,
            token: token,
            read_state: ReadState::Header(0),
            buffer: [0; MAX_PACKET_SIZE],
            buffer_state: BufferState { read_index: 0, write_index: 0 }
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    pub fn read(&mut self, event_loop: &mut mio::EventLoop<PubsubServer>) -> ClientAction {
        let action = match self.socket.try_read(&mut self.buffer[self.buffer_state.write_index..]) {
            Ok(Some(len)) => {
                self.buffer_state.write_index += len;
                self.handle_read(len)
            },
            Ok(None) => {
                // TODO: what exactly does this imply?
                println!("Got None while reading");
                ClientAction::Nothing
            },
            Err(_) => { return ClientAction::Error; }
        };
        event_loop.reregister(&self.socket, self.token, EventSet::readable(),
                              PollOpt::edge() | PollOpt::oneshot()).unwrap();
        action
    }

    pub fn write() {
        unimplemented!();
    }

    fn handle_read(&mut self, read_len: usize) -> ClientAction {
        let mut packet_complete = false;

        let action = match self.read_state {
            ReadState::Header(in_buffer) => {
                let parse_result = parse(&self.buffer[..in_buffer + read_len]);
                match parse_result {
                    ParseResult::Completed(msg_header, header_len, payload_len) => {
                        self.buffer_state.read_index = header_len;

                        assert!(in_buffer + read_len >= header_len);
                        let remaining_in_buffer = in_buffer + read_len - header_len;

                        match self.on_header(msg_header, remaining_in_buffer, payload_len) {
                            Some(action) => {
                                packet_complete = true;
                                action
                            },
                            None => ClientAction::Nothing
                        }
                    },
                    ParseResult::Incomplete => {
                        self.read_state = ReadState::Header(in_buffer + read_len);
                        ClientAction::Nothing
                    }
                    ParseResult::Error => {
                        packet_complete = true;
                        ClientAction::Error
                    }
                }
            },
            ReadState::Payload(ref header, ref mut in_buffer, payload_len) => {
                if *in_buffer + read_len >= payload_len {
                    let payload = &self.buffer[self.buffer_state.read_index..self.buffer_state.read_index + payload_len];
                    // TODO! Handle extra remaining that is not part of the payload, i.e. next packet
                    packet_complete = true;
                    ClientAction::Publish(header.event_name.clone(), Vec::from(payload))
                }
                else {
                    *in_buffer += read_len;
                    ClientAction::Nothing
                }
            }
        };

        if packet_complete {
            self.on_packet_complete();
        }

        action
    }

    fn on_header(&mut self, header: MessageHeader, remaining_in_buffer: usize, payload_len: usize)
                 -> Option<ClientAction> {
        use pubsub::message::MessageType::*;

        match header.message_type {
            Subscribe => Some(ClientAction::Subscribe(header.event_name)),
            Unsubscribe => Some(ClientAction::Unsubscribe(header.event_name)),
            Publish => {
                if remaining_in_buffer >= payload_len {
                    // Got the entire payload as well in the same read
                    let payload = &self.buffer[self.buffer_state.read_index..self.buffer_state.read_index + payload_len];
                    // TODO! Handle extra remaining that is not part of the payload, i.e. next packet
                    Some(ClientAction::Publish(header.event_name, Vec::from(payload)))
                }
                else {
                    self.read_state = ReadState::Payload(header, remaining_in_buffer, payload_len);
                    None
                }
            },
            // Event should only be sent server -> client
            Event => Some(ClientAction::Error)
        }
    }

    fn on_packet_complete(&mut self) {
        self.read_state = ReadState::Header(0);
        self.buffer_state.reset();
    }
}
