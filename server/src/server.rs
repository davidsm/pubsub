use mio::tcp::{TcpListener};
use mio::util::Slab;
use mio::{EventSet, PollOpt};
use mio;

use client::{PubsubClient, ClientAction};

use subscriptions::{SubscriptionMap, ClientMap};

pub const SERVER_TOKEN: mio::Token = mio::Token(0);

pub struct PubsubServer {
    socket: TcpListener,
    connections: Slab<PubsubClient>,
    subscriptions: SubscriptionMap
}

impl PubsubServer {
    pub fn new(socket: TcpListener, subscription_map: SubscriptionMap) -> PubsubServer {
        PubsubServer {
            socket: socket,
            connections: Slab::new_starting_at(mio::Token(1), 128),
            subscriptions: subscription_map
        }
    }
}

impl mio::Handler for PubsubServer {
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
                let mut action = self.connections[token].read(event_loop);
                // Might get more than one packet in a read, so loop
                // until there are no more complete packets
                loop {
                    match action {
                        ClientAction::Nothing => {
                            println!("No action. Break read loop");
                            break;
                        },
                        ClientAction::Subscribe(event) => {
                            println!("Subscribe to {}", event);
                            let client_map = self.subscriptions.entry(event)
                                .or_insert(ClientMap::new());
                            client_map.insert(token);
                        },
                        ClientAction::Unsubscribe(event) => {
                            println!("Unsubscribe to {}", event);
                            if let Some(client_map) = self.subscriptions.get_mut(&event) {
                                client_map.remove(&token);
                            }
                        },
                        ClientAction::Publish(event, payload) => {
                            println!("Publish {} to {}", String::from_utf8(payload).unwrap(), event);
                        },
                        ClientAction::Error => {
                            println!("Error!");
                            break;
                        }
                    }
                    action = self.connections[token].handle_read(0);
                }
            }
        }
    }
}
