use std::collections::{HashMap, HashSet};

use mio;

pub type ClientMap = HashSet<mio::Token>;
pub type SubscriptionMap = HashMap<String, ClientMap>;
