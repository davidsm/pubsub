use std::collections::HashMap;

use mio;
use client::PubsubClient;

pub type ClientMap<'a> = HashMap<mio::Token, &'a PubsubClient>;
pub type SubscriptionMap<'a> = HashMap<String, ClientMap<'a>>;
