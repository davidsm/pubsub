use byteorder::{BigEndian, ReadBytesExt};
use std::io;

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
    partial_message: MessageBuilder
}

impl<'a> Parser {
    pub fn new() -> Parser {
        Parser {
            awaiting: Awaiting::MessageType,
            partial_message: MessageBuilder::new()
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> ParseResult<'a> {
        let mut remainder = data;
        match self.awaiting {
            Awaiting::MessageType => {
                let (message_type, rest) = try_parse!(self.read_message_type(remainder));

                self.partial_message.message_type(message_type);
                self.awaiting = Awaiting::EventNameLen;
                remainder = rest;
            },
            _ => {}
        };

        ParseResult::Incomplete
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
}