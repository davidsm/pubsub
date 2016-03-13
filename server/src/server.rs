use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;
use mio::{EventSet, PollOpt, TryRead};
use mio;

use std::str;

const SERVER: mio::Token = mio::Token(0);

struct PubsubClient {
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

    pub fn ready(&mut self, event_loop: &mut mio::EventLoop<PubsubServer>) {
        let mut buf = [0; 128];
        match self.socket.try_read(&mut buf) {
            Ok(Some(len)) => println!("{:?}", str::from_utf8(&buf[..len]).unwrap()),
            Ok(None) => println!("Got None while reading"),
            Err(e) => println!("{}", e)
        }
        event_loop.reregister(&self.socket, self.token, EventSet::readable(),
                              PollOpt::edge() | PollOpt::oneshot()).unwrap();
    }
}

pub struct PubsubServer {
    socket: TcpListener,
    connections: Slab<PubsubClient>
}

impl PubsubServer {
    pub fn new(socket: TcpListener) -> PubsubServer {
        PubsubServer {
            socket: socket,
            connections: Slab::new_starting_at(mio::Token(1), 128)
        }
    }
}

impl mio::Handler for PubsubServer {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut mio::EventLoop<PubsubServer>,
             token: mio::Token, events: mio::EventSet) {
        match token {
            SERVER => {
                let client_socket = match self.socket.accept() {
                    Err(e) => {
                        println!("{}", e);
                        return;
                    },
                    Ok(None) => {
                        println!("Couldn't accept connection (None)");
                        return;
                    }
                    Ok(Some((socket, address))) => {
                        println!("Got a connection from {}", address);
                        socket
                    }
                };

                let token = self.connections.insert_with(|token| {
                    PubsubClient::new(client_socket, token)
                }).expect("Failed to insert new connection");

                event_loop.register(self.connections[token].socket(), token,
                                    EventSet::readable(),
                                    PollOpt::edge() | PollOpt::oneshot())
                    .expect("Failed to register new connection with event loop");

            },

            _ => {
                self.connections[token].ready(event_loop);
            }
        }
    }
}
