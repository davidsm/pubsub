use std::collections::HashMap;

use mio;

pub type EventId = mio::Token;

pub struct PendingEvents {
    event_map: HashMap<EventId, Event>,
    id_counter: usize
}

impl PendingEvents {
    pub fn new() -> PendingEvents {
        PendingEvents {
            event_map: HashMap::new(),
            id_counter: 1
        }
    }

    pub fn add_event(&mut self, data: Vec<u8>, recipients: usize) -> EventId {
        let event_id = mio::Token(self.id_counter);
        // This is fairly ugly, but wrapping around after sizeof(usize) events
        // is very unlikely to cause issues, and it's better to be explicit
        // about that eventually happening (plus integer overflow behaves differently
        // in debug and release mode in rust...)
        self.id_counter = self.id_counter.wrapping_add(1);
        self.event_map.insert(event_id, Event::new(data, recipients));
        event_id
    }

    pub fn get_event_data(&self, event_id: EventId) -> Option<&[u8]> {
        self.event_map.get(&event_id)
            .map(|e| e.data())
    }

    pub fn finish_event(&mut self, event_id: EventId) {
        if self.event_map.get_mut(&event_id)
            .and_then(|event| event.remove_recipient())
            .is_none() {
                self.event_map.remove(&event_id);
            }
    }
}

struct Event {
    remaining_recipients: usize,
    data: Vec<u8>
}

impl Event {
    fn new(data: Vec<u8>, recipients: usize) -> Event {
        if recipients == 0 {
            panic!("Recipients must be non-zero");
        }
        Event {
            data: data,
            remaining_recipients: recipients
        }
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn remove_recipient(&mut self) -> Option<usize> {
        self.remaining_recipients -= 1;
        if self.remaining_recipients > 0 {
            Some(self.remaining_recipients)
        }
        else {
            None
        }
    }
}
