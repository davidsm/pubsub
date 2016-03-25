use mio::tcp::{TcpListener};
use mio::util::Slab;
use mio::{EventSet, PollOpt};
use mio;

use client::{PubsubClient, ClientAction};

use subscriptions::SubscriptionMap;

const SERVER_TOKEN: mio::Token = mio::Token(0);

pub struct PubsubServer<'a> {
    socket: TcpListener,
    connections: Slab<PubsubClient>,
    subscriptions: SubscriptionMap<'a>
}

impl<'a> PubsubServer<'a> {
    pub fn new(socket: TcpListener, subscription_map: SubscriptionMap) -> PubsubServer {
        PubsubServer {
            socket: socket,
            connections: Slab::new_starting_at(mio::Token(1), 128),
            subscriptions: subscription_map
        }
    }

    pub fn token() -> mio::Token {
        SERVER_TOKEN
    }
}

impl<'a> mio::Handler for PubsubServer<'a> {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut mio::EventLoop<PubsubServer>,
             token: mio::Token, events: mio::EventSet) {
        match token {
            SERVER_TOKEN => {
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
                match self.connections[token].read(event_loop) {
                    ClientAction::Nothing => {},
                    _ => unimplemented!()
                }
            }
        }
    }
}
