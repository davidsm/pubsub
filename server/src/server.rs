use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;
use mio::{EventSet, PollOpt};
use mio;

const SERVER: mio::Token = mio::Token(0);

struct Connection {
    socket: TcpStream,
    token: mio::Token
}

impl Connection {
    pub fn new(socket: TcpStream, token: mio::Token) -> Connection {
        Connection {
            socket: socket,
            token: token
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }
}

pub struct PubsubServer {
    socket: TcpListener,
    connections: Slab<Connection>
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
                    Connection::new(client_socket, token)
                }).expect("Failed to insert new connection");

                event_loop.register(self.connections[token].socket(), token,
                                    EventSet::readable(),
                                    PollOpt::edge() | PollOpt::oneshot())
                    .expect("Failed to register new connection with event loop");

            },

            _ => unreachable!()
        }
    }
}
