use std::u8;
use std::u16;

use byteorder::{BigEndian, WriteBytesExt};

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum MessageType {
    Subscribe = 1,
    Unsubscribe,
    Publish,
    Event
}

impl MessageType {
    pub fn expects_payload(&self) -> bool {
        match *self {
            MessageType::Subscribe | MessageType::Unsubscribe => false,
            MessageType::Publish | MessageType::Event => true
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct MessageHeader {
    pub message_type: MessageType,
    pub event_name: String,
}

#[derive(PartialEq, Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Option<Vec<u8>>
}

impl Message {
    pub fn into_bytes(self) -> Vec<u8> {
        let mut vec = Vec::<u8>::new();
        // Should be safe to use unwrap, as writing to a Vec should not fail
        vec.write_u8(self.header.message_type as u8).unwrap();
        vec.write_u8(self.header.event_name.len() as u8).unwrap();
        vec.extend(self.header.event_name.into_bytes());
        if let Some(payload) = self.payload {
            vec.write_u16::<BigEndian>(payload.len() as u16).unwrap();
            vec.extend(payload);
        }
        vec
    }
}

#[derive(PartialEq, Debug, Default)]
pub struct MessageBuilder {
    message_type: Option<MessageType>,
    event_name: Option<String>,
    payload: Option<Vec<u8>>
}

impl MessageBuilder {
    pub fn new() -> MessageBuilder {
        MessageBuilder {
            message_type: None,
            event_name: None,
            payload: None
        }
    }

    pub fn message_type(&mut self, message_type: MessageType) -> &mut MessageBuilder {
        self.message_type = Some(message_type);
        self
    }

    pub fn event_name(&mut self, event_name: String) -> &mut MessageBuilder {
        self.event_name = Some(event_name);
        self
    }

    pub fn payload(&mut self, payload: Vec<u8>) -> &mut MessageBuilder {
        self.payload = Some(payload);
        self
    }

    pub fn validate(&self, only_header: bool) -> Result<(), MessageBuildError> {
        let mut missing_fields = Vec::new();
        if self.message_type.is_none() {
            missing_fields.push("message type");
        }
        if self.event_name.is_none() {
            missing_fields.push("event name")
        }
        if !missing_fields.is_empty() {
            return Err(MessageBuildError::MissingField(missing_fields.join(", ")));
        }

        if self.event_name.as_ref().unwrap().len() > u8::MAX as usize {
            return Err(MessageBuildError::TooLargeField(String::from("event name")));
        }

        if only_header {
            return Ok(());
        }

        if self.message_type.unwrap().expects_payload() {
            if self.payload.is_none() {
                return Err(MessageBuildError::MissingField(String::from("payload")));
            }
            if self.payload.as_ref().unwrap().len() > u16::MAX as usize {
                return Err(MessageBuildError::TooLargeField(String::from("payload")));
            }
        }
        else if self.payload.is_some() {
            return Err(MessageBuildError::InvalidField(
                String::from("payload")));
        }

        Ok(())
    }

    pub fn build_header(self) -> Result<MessageHeader, MessageBuildError> {
        try!(self.validate(true));
        Ok(MessageHeader {
            message_type: self.message_type.unwrap(),
            event_name: self.event_name.unwrap()
        })
    }

    pub fn build(self) -> Result<Message, MessageBuildError> {
        try!(self.validate(false));
        let header = MessageHeader {
            message_type: self.message_type.unwrap(),
            event_name: self.event_name.unwrap()
        };
        let message = Message {
            header: header,
            payload: self.payload
        };
        Ok(message)
    }
}

#[derive(PartialEq, Debug)]
pub enum MessageBuildError {
    MissingField(String),
    TooLargeField(String),
    InvalidField(String)
}

#[cfg(test)]
mod test {
    use super::{Message, MessageHeader, MessageBuilder, MessageBuildError, MessageType};

    #[test]
    fn test_into_bytes() {
        let message = Message {
            header: MessageHeader {
                message_type: MessageType::Publish,
                event_name: "event".to_string(),
            },
            payload: Some("a payload here".to_string().into_bytes())
        };

        let expected_bytes = vec![
            0x03, // Type
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name
            0x00, 0x0E, // Payload length
            0x61, 0x20, 0x70, 0x61, 0x79,// Payload
            0x6c, 0x6f, 0x61, 0x64, 0x20,
            0x68, 0x65, 0x72, 0x65
                ];
        assert_eq!(message.into_bytes(), expected_bytes);
    }

    #[test]
    fn test_validate_builder_no_fields() {
        let builder = MessageBuilder::new();
        assert_eq!(builder.build(),
                   Err(MessageBuildError::MissingField("message type, event name".to_string())));
    }

    #[test]
    fn test_validate_builder_no_message_type() {
        let mut builder = MessageBuilder::new();
        builder.event_name("event".to_string());
        assert_eq!(builder.build(),
                   Err(MessageBuildError::MissingField("message type".to_string())));
    }

    #[test]
    fn test_validate_builder_no_event_name() {
        let mut builder = MessageBuilder::new();
        builder.message_type(MessageType::Subscribe);
        assert_eq!(builder.build(),
                   Err(MessageBuildError::MissingField("event name".to_string())));
    }

    #[test]
    fn test_validate_builder_event_name_length() {
        let mut builder = MessageBuilder::new();
        let string_bytes = vec![97; 256];
        let event_name = String::from_utf8(string_bytes).unwrap();
        builder.message_type(MessageType::Subscribe).
            event_name(event_name);
        assert_eq!(builder.build(),
                   Err(MessageBuildError::TooLargeField("event name".to_string())));
    }

    #[test]
    fn test_validate_builder_payload_length() {
        let mut builder = MessageBuilder::new();
        let payload = vec![0xAB; 65536];
        builder.message_type(MessageType::Publish).
            event_name("event".to_string()).
            payload(payload);
        assert_eq!(builder.build(),
                   Err(MessageBuildError::TooLargeField("payload".to_string())));

    }

    #[test]
    fn test_validate_builder_has_payload() {
        for message_type in vec![MessageType::Subscribe, MessageType::Unsubscribe] {
            let mut builder = MessageBuilder::new();
            builder.message_type(message_type).
                event_name("event".to_string()).
                payload("a payload".to_string().into_bytes());
            assert_eq!(builder.build(),
                       Err(MessageBuildError::InvalidField("payload".to_string())));
        }
        for message_type in vec![MessageType::Publish, MessageType::Event] {
            let mut builder = MessageBuilder::new();
            builder.message_type(message_type).
                event_name("event".to_string()).
                payload("a payload".to_string().into_bytes());
            assert!(builder.build().is_ok());
        }
    }
}
