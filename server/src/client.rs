use mio;
use mio::tcp::{TcpStream};
use mio::{EventSet, PollOpt, TryRead, TryWrite};

use pubsub::message::MessageHeader;
use pubsub::parser::{parse, ParseResult};

use server::{EventLoop};

use pending_event::{EventId, PendingEvents};

use std::{u8, u16};

use std::collections::VecDeque;


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

struct Buffer {
    buf: [u8; MAX_PACKET_SIZE],
    read_index: usize,
    write_index: usize
}

impl Buffer {
    fn new() -> Buffer {
        Buffer {
            buf: [0; MAX_PACKET_SIZE],
            read_index: 0,
            write_index: 0
        }
    }

    fn reset(&mut self) {
        self.read_index = 0;
        self.write_index = 0;
    }

    fn reshuffle(&mut self, remaining_bytes: usize) {
        let mut i = 0;
        let start = self.write_index - remaining_bytes;
        for j in start..self.write_index {
            self.buf[i] = self.buf[j];
            i += 1;
        }
        self.read_index = 0;
        self.write_index = remaining_bytes;
    }

    fn writable(&mut self) -> &mut [u8] {
        &mut self.buf[self.write_index..]
    }

    fn read_bytes(&self, bytes: usize) -> &[u8] {
        &self.buf[self.read_index..self.read_index + bytes]
    }

    fn unused_bytes(&self) -> usize {
        assert!(self.write_index >= self.read_index);
        self.write_index - self.read_index
    }
}

struct WriteQueue {
    queue: VecDeque<EventId>,
    write_index: usize
}

impl WriteQueue {
    fn new() -> WriteQueue {
        WriteQueue {
            queue: VecDeque::new(),
            write_index: 0
        }
    }

    fn add_event(&mut self, event_id: EventId) {
        self.queue.push_back(event_id);
    }

    fn current_event_id(&self) -> EventId {
        *self.queue.get(0).unwrap()
    }

    fn has_events_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    fn finish_current_event(&mut self) -> EventId {
        let event_id = self.queue.pop_front()
            .expect("Called finish_current_event with no events left");
        self.write_index = 0;
        event_id
    }
}

pub struct PubsubClient {
    socket: TcpStream,
    token: mio::Token,
    read_state: ReadState,
    write_queue: WriteQueue,
    buffer: Buffer
}

impl PubsubClient {
    pub fn new(socket: TcpStream, token: mio::Token) -> PubsubClient {
        PubsubClient {
            socket: socket,
            token: token,
            read_state: ReadState::Header(0),
            write_queue: WriteQueue::new(),
            buffer: Buffer::new()
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    fn reregister(&self, event_loop: &mut EventLoop) {
        let mut event_set = EventSet::readable();
        if self.write_queue.has_events_pending() {
            event_set = event_set | EventSet::writable();
        }
        event_loop.reregister(&self.socket, self.token, event_set,
                              PollOpt::edge() | PollOpt::oneshot()).unwrap();
    }

    pub fn read(&mut self, event_loop: &mut EventLoop) -> ClientAction {
        let action = match self.socket.try_read(self.buffer.writable()) {
            Ok(Some(0)) => { return ClientAction::Error },
            Ok(Some(len)) => {
                println!("Read {} bytes", len);
                self.buffer.write_index += len;
                self.handle_read(len)
            },
            Ok(None) => {
                // Would Block. Do nothing and simply try again later
                ClientAction::Nothing
            },
            Err(_) => { return ClientAction::Error; }
        };
        self.reregister(event_loop);
        action
    }

    pub fn write(&mut self, event_loop: &mut EventLoop, pending_events: &mut PendingEvents) -> Result<(), ()> {
        // Ugly way of limiting the lifetime of "data"
        // in order to be able to mutably borrow "pending_events" further down
        let (write_res, data_len) = {
            let data = match pending_events.get_event_data(self.write_queue.current_event_id()) {
                Some(d) => &d[self.write_queue.write_index..],
                None => {
                    println!("Tried to get data for non existing event in write! Should not happen");
                    return Err(());
                }
            };
            (self.socket.try_write(data), data.len())
        };

        match write_res {
            // Is this OK or not, i.e. should the client be disconnected?
            Ok(Some(0)) => { return Err(()) },
            Ok(Some(len)) => {
                self.write_queue.write_index += len;
                if self.write_queue.write_index >= data_len {
                    self.finish_event(pending_events);
                }
            },
            // Would block. Try again later
            Ok(None) => {}
            Err(_) => { return Err(()) }
        }
        self.reregister(event_loop);
        Ok(())
    }

    pub fn handle_read(&mut self, read_len: usize) -> ClientAction {
        let mut packet_complete = false;

        let action = match self.read_state {
            ReadState::Header(in_buffer) => {
                let readable_bytes = in_buffer + read_len;
                if readable_bytes == 0 {
                    return ClientAction::Nothing;
                }
                let parse_result = parse(self.buffer.read_bytes(readable_bytes));
                match parse_result {
                    ParseResult::Completed(msg_header, header_len, payload_len) => {
                        self.buffer.read_index = header_len;

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
                    let payload = Vec::from(self.buffer.read_bytes(payload_len));
                    self.buffer.read_index += payload_len;
                    packet_complete = true;
                    ClientAction::Publish(header.event_name.clone(), payload)
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

    pub fn publish(&mut self, event_id: EventId, event_loop: &mut EventLoop) {
        self.write_queue.add_event(event_id);
        self.reregister(event_loop);
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
                    let payload = Vec::from(self.buffer.read_bytes(payload_len));
                    self.buffer.read_index += payload_len;
                    Some(ClientAction::Publish(header.event_name, payload))
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
        let unused_bytes = self.buffer.unused_bytes();
        if unused_bytes > 0 {
            self.buffer.reshuffle(unused_bytes);
        }
        else {
            self.buffer.reset();
        }
        self.read_state = ReadState::Header(unused_bytes);
    }

    fn finish_event(&mut self, pending_events: &mut PendingEvents) {
        let event_id = self.write_queue.finish_current_event();
        pending_events.finish_event(event_id);
    }

    pub fn clear_events(&mut self, pending_events: &mut PendingEvents) {
        while self.write_queue.has_events_pending() {
            self.finish_event(pending_events)
        }
    }
}
