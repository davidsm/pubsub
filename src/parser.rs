use byteorder::{BigEndian, ReadBytesExt};
use std::io;
use std::str;
use std::mem;

use message::{MessageBuilder, Message, MessageType};

macro_rules! try_parse {
    ($expr:expr) => (match $expr {
        Result::Ok(val) => val,
        Result::Err(err) => {
            return ParseResult::from(err)
        }
    })
}

pub fn take_n(n: usize, b: &[u8]) -> Option<(&[u8], &[u8])> {
    if b.len() < n {
        None
    }
    else {
        Some((&b[0..n], &b[n..]))
    }
}

pub fn read_u8(b: &[u8]) -> Option<(u8, &[u8])> {
    let mut c = io::Cursor::new(b);
    match c.read_u8() {
        Ok(n) => Some((n, &b[1..])),
        Err(_) => None
    }
}

pub fn read_u16(b: &[u8]) -> Option<(u16, &[u8])> {
    let mut c = io::Cursor::new(b);
    match c.read_u16::<BigEndian>() {
        Ok(n) => Some((n, &b[2..])),
        Err(_) => None
    }
}

#[derive(PartialEq, Debug)]
pub enum ParseResult<'a> {
    Completed(Message, &'a [u8]),
    Incomplete,
    Error
}

impl<'a> From<ParserError> for ParseResult<'a> {
    fn from(err: ParserError) -> ParseResult<'a> {
        match err {
            ParserError::InsufficientData => ParseResult::Incomplete,
            ParserError::InvalidValue => ParseResult::Error
        }
    }
}

#[derive(PartialEq, Debug)]
enum Awaiting {
    MessageType,
    EventNameLen,
    EventName,
    PayloadLen,
    Payload
}

#[derive(PartialEq, Debug)]
pub enum ParserError {
    InsufficientData,
    InvalidValue
}

pub struct Parser {
    awaiting: Awaiting,
    partial_message: MessageBuilder,
    current_message_type: Option<MessageType>,
    expected_event_name_len: Option<u8>,
    expected_payload_len: Option<u16>
}

impl<'a> Parser {
    pub fn new() -> Parser {
        Parser {
            awaiting: Awaiting::MessageType,
            partial_message: MessageBuilder::new(),
            current_message_type: None,
            expected_event_name_len: None,
            expected_payload_len: None
        }
    }

    pub fn feed(&mut self, data: &'a [u8]) -> ParseResult<'a> {
        let mut remainder = data;
        while remainder.len() > 0 {
            match self.awaiting {
                Awaiting::MessageType => {
                    let (message_type, rest) = try_parse!(self.read_message_type(remainder));
                    self.partial_message.message_type(message_type);
                    self.current_message_type = Some(message_type);
                    self.awaiting = Awaiting::EventNameLen;
                    remainder = rest;
                },
                Awaiting::EventNameLen => {
                    let (len, rest) = try_parse!(read_u8(remainder)
                                                 .ok_or(ParserError::InsufficientData));
                    self.expected_event_name_len = Some(len);
                    self.awaiting = Awaiting::EventName;
                    remainder = rest;
                },
                Awaiting::EventName => {
                    let (event_name, rest) = try_parse!(self.read_event_name(remainder));
                    self.partial_message.event_name(event_name);
                    remainder = rest;
                    if self.current_message_type.unwrap().expects_payload() {
                        self.awaiting = Awaiting::PayloadLen;
                    }
                    else {
                        let message = try_parse!(self.complete_parse());
                        return ParseResult::Completed(message, remainder);
                    }
                },
                Awaiting::PayloadLen => {
                    let (len, rest) = try_parse!(read_u16(remainder)
                                                 .ok_or(ParserError::InsufficientData));
                    self.expected_payload_len = Some(len);
                    self.awaiting = Awaiting::Payload;
                    remainder = rest;
                },
                Awaiting::Payload => {
                    assert!(self.expected_payload_len.is_some());
                    let (payload_bytes, rest) = try_parse!(
                        take_n(self.expected_payload_len.unwrap() as usize, remainder)
                            .ok_or(ParserError::InsufficientData));
                    self.partial_message.payload(payload_bytes.to_vec());
                    let message = try_parse!(self.complete_parse());
                    return ParseResult::Completed(message, rest);
                }
            };
        }
        ParseResult::Incomplete
    }

    fn complete_parse(&mut self) -> Result<Message, ParserError> {
        let partial_message = mem::replace(&mut self.partial_message, MessageBuilder::new());
        self.awaiting = Awaiting::MessageType;
        self.current_message_type = None;
        self.expected_event_name_len = None;
        self.expected_payload_len = None;

        let message = try!(partial_message.build()
                           .or(Err(ParserError::InvalidValue)));
        Ok(message)
    }

    fn match_message_type(&self, val: u8) -> Result<MessageType, ParserError> {
        // Need to use clunky if statements here,
        // as match doesn't allow you to cast the enum value
        if val == MessageType::Subscribe as u8 {
            Ok(MessageType::Subscribe)
        }
        else if val == MessageType::Unsubscribe as u8 {
            Ok(MessageType::Unsubscribe)
        }
        else if val == MessageType::Publish as u8 {
            Ok(MessageType::Publish)
        }
        else if val == MessageType::Event as u8 {
            Ok(MessageType::Event)
        }
        else {
            Err(ParserError::InvalidValue)
        }
    }

    fn read_message_type<'b>(&self, b: &'b [u8])
                             -> Result<(MessageType, &'b [u8]), ParserError> {
        match read_u8(b) {
            Some((val, remainder)) => {
                let message_type = try!(self.match_message_type(val));
                Ok((message_type, remainder))
            },
            None => Err(ParserError::InsufficientData)
        }
    }

    fn read_event_name<'b>(&self, b: &'b [u8])
                           -> Result<(String, &'b [u8]), ParserError> {
        assert!(self.expected_event_name_len.is_some());
        let (event_name_bytes, rest) = try!(take_n(self.expected_event_name_len.unwrap() as usize, b)
                                            .ok_or(ParserError::InsufficientData));
        let event_name = try!(str::from_utf8(event_name_bytes)
                              .or(Err(ParserError::InvalidValue))).to_string();
        Ok((event_name, rest))

    }
}

#[cfg(test)]
mod test {
    use super::*;
    use super::{Awaiting};
    use message::MessageType;
    use std::borrow::Borrow;

    #[test]
    fn test_read_u8() {
        let buf = vec![0, 1, 2];
        let mut slice = buf.borrow();
        let mut counter = 0;
        for _ in 0..buf.len() {
            let res = read_u8(slice);
            assert!(res.is_some());
            let (byte, remainder) = res.unwrap();
            assert_eq!(byte, counter);
            slice = remainder;
            counter += 1;
        }

        assert!(slice.is_empty());
        let res = read_u8(slice);
        assert!(res.is_none());
    }

    #[test]
    fn test_read_u16() {
        let buf = vec![0xBE, 0xEF, 0xFE, 0xED];
        let mut slice = buf.borrow();
        let expected = [48879, 65261];

        for i in 0..2 {
            let res = read_u16(slice);
            assert!(res.is_some());
            let (value, remainder) = res.unwrap();
            assert_eq!(value, expected[i]);
            slice = remainder;
        }
        assert!(slice.is_empty());
        let res = read_u16(slice);
        assert!(res.is_none());
    }

    #[test]
    fn test_read_u16_incomplete() {
        let buf = [0xAB];
        let res = read_u16(&buf);
        assert!(res.is_none());
    }

    #[test]
    fn test_match_message_type() {
        let p = Parser::new();
        let expected_pairs = vec![(1, MessageType::Subscribe),
                                  (2, MessageType::Unsubscribe),
                                  (3, MessageType::Publish),
                                  (4, MessageType::Event)];
        for (val, message_type) in expected_pairs {
            assert_eq!(p.match_message_type(val), Ok(message_type));
        }
        assert_eq!(p.match_message_type(5), Err(ParserError::InvalidValue));
    }

    #[test]
    fn test_read_message_type() {
        let bytes = [MessageType::Subscribe as u8];
        let mut p = Parser::new();
        let res = p.feed(&bytes);
        assert_eq!(p.awaiting, Awaiting::EventNameLen);
        assert_eq!(res, ParseResult::Incomplete);
    }

    #[test]
    fn test_parse_complete_message() {
        let message_bytes = vec![
            0x03, // Type (Publish)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name (event)
            0x00, 0x0E, // Payload length
            0x61, 0x20, 0x70, 0x61, 0x79,// Payload (a payload here)
            0x6c, 0x6f, 0x61, 0x64, 0x20,
            0x68, 0x65, 0x72, 0x65
                ];
        let mut p = Parser::new();
        let res = p.feed(&message_bytes);
        assert!(res != ParseResult::Incomplete);
        assert!(res != ParseResult::Error);
        if let ParseResult::Completed(message, remainder) = res {
            assert_eq!(message.message_type, MessageType::Publish);
            assert_eq!(message.event_name, "event".to_string());
            let expected_payload: Vec<u8> = vec![0x61, 0x20, 0x70, 0x61, 0x79,
                                                 0x6c, 0x6f, 0x61, 0x64, 0x20,
                                                 0x68, 0x65, 0x72, 0x65];
            assert_eq!(message.payload, Some(expected_payload));
            assert_eq!(remainder.len(), 0);
        }
        else {
            assert!(false, "Result wasn't Completed");
        }
    }
}
