use mio;
use mio::tcp::{TcpStream};
use mio::{EventSet, PollOpt, TryRead};

use pubsub::message;
use pubsub::parser::parse;

use server::PubsubServer;

use std::str;

pub enum ClientAction {
    Subscribe(String),
    Publish(message::Message),
    Unsubscribe(String),
    Nothing
}

pub struct PubsubClient {
    socket: TcpStream,
    token: mio::Token
}

impl PubsubClient {
    pub fn new(socket: TcpStream, token: mio::Token) -> PubsubClient {
        PubsubClient {
            socket: socket,
            token: token
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    pub fn read(&mut self, event_loop: &mut mio::EventLoop<PubsubServer>) -> ClientAction {
        let mut buf = [0; 128];
        match self.socket.try_read(&mut buf) {
            Ok(Some(len)) => println!("{:?}", str::from_utf8(&buf[..len]).unwrap()),
            Ok(None) => println!("Got None while reading"),
            Err(e) => println!("{}", e)
        }
        event_loop.reregister(&self.socket, self.token, EventSet::readable(),
                              PollOpt::edge() | PollOpt::oneshot()).unwrap();
        ClientAction::Nothing
    }

    pub fn write() {
        unimplemented!();
    }
}
