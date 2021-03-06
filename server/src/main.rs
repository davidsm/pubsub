extern crate pubsub;
extern crate mio;

mod server;
use server::{PubsubServer, SERVER_TOKEN};

mod subscriptions;

mod client;
mod pending_event;

use mio::{EventLoop, EventSet, PollOpt};
use mio::tcp::TcpListener;


fn main() {
    let bind_address = "127.0.0.1:9876".parse().unwrap();
    let listener = TcpListener::bind(&bind_address).unwrap();

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&listener, SERVER_TOKEN, EventSet::readable(), PollOpt::edge()).unwrap();
    event_loop.run(&mut PubsubServer::new(listener)).unwrap();
}
