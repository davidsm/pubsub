use mio::tcp::{TcpListener};
use mio::util::Slab;
use mio::{EventSet, PollOpt};
use mio;

use pubsub::message::{MessageType, MessageBuilder};

use client::{PubsubClient, ClientAction};

use subscriptions::{SubscriptionMap, ClientMap};
use pending_event::PendingEvents;


pub const SERVER_TOKEN: mio::Token = mio::Token(0);

pub type EventLoop = mio::EventLoop<PubsubServer>;

pub struct PubsubServer {
    socket: TcpListener,
    connections: Slab<PubsubClient>,
    subscriptions: SubscriptionMap,
    pending_events: PendingEvents
}

impl PubsubServer {
    pub fn new(socket: TcpListener, subscription_map: SubscriptionMap) -> PubsubServer {
        PubsubServer {
            socket: socket,
            connections: Slab::new_starting_at(mio::Token(1), 128),
            subscriptions: subscription_map,
            pending_events: PendingEvents::new()
        }
    }

    fn on_client_connection(&mut self, event_loop: &mut EventLoop) {
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

    }

    fn on_client_readable(&mut self, event_loop: &mut EventLoop, token: mio::Token) {
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
                        // TODO: Remove key if no subscriptions left
                    }
                },
                ClientAction::Publish(event, payload) => {
                    println!("Publish {} to {}", String::from_utf8(payload.clone()).unwrap(), event);
                    if let Some(clients) = self.subscriptions.get(&event) {
                        let mut builder = MessageBuilder::new();
                        builder.message_type(MessageType::Event)
                            .event_name(event)
                            .payload(payload);
                        let message_data = match builder.build() {
                            Ok(msg) => msg.to_bytes(),
                            Err(_) => break
                        };

                        let event_id = self.pending_events.add_event(message_data,
                                                                     clients.len());
                        for client_token in clients {
                            self.connections[*client_token].publish(event_id, event_loop);
                        }
                    }
                },
                ClientAction::Error => {
                    println!("Error!");
                    self.connections.remove(token);
                    // TODO: Remove subscriptions also
                    break;
                }
            }
            action = self.connections[token].handle_read(0);
        }
    }

    fn on_client_writable(&mut self, event_loop: &mut EventLoop, token: mio::Token) {
        self.connections[token].write(event_loop, &mut self.pending_events);
    }
}

impl mio::Handler for PubsubServer {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop,
             token: mio::Token, events: mio::EventSet) {
        match token {
            SERVER_TOKEN => {
                self.on_client_connection(event_loop);
            },
            _ => {
                if events.is_readable() {
                    self.on_client_readable(event_loop, token);
                }
                if events.is_writable() {
                    self.on_client_writable(event_loop, token);
                }
            }
        }
    }
}
