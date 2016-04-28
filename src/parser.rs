use byteorder::{BigEndian, ReadBytesExt};
use std::io;
use std::str;

use message::{MessageBuilder, MessageType, MessageHeader};

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
pub enum ParseResult {
    Completed(MessageHeader, usize, usize),
    Incomplete,
    Error
}

impl From<ParserError> for ParseResult {
    fn from(err: ParserError) -> ParseResult {
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
    PayloadLen
}

#[derive(PartialEq, Debug)]
pub enum ParserError {
    InsufficientData,
    InvalidValue
}


pub fn parse(data: &[u8]) -> ParseResult {
    let mut awaiting = Awaiting::MessageType;
    let mut partial_message = MessageBuilder::new();

    let mut current_message_type = None;
    let mut expected_event_name_len = None;

    let mut consumed: usize = 0;

    let mut remainder = data;

    while remainder.len() > 0 {
        match awaiting {
            Awaiting::MessageType => {
                let (message_type, rest) = try_parse!(read_message_type(remainder));
                partial_message.message_type(message_type);
                current_message_type = Some(message_type);
                awaiting = Awaiting::EventNameLen;
                consumed += remainder.len() - rest.len();
                remainder = rest;
            },
            Awaiting::EventNameLen => {
                let (len, rest) = try_parse!(read_u8(remainder)
                                             .ok_or(ParseResult::Incomplete));
                expected_event_name_len = Some(len);
                awaiting = Awaiting::EventName;
                consumed += remainder.len() - rest.len();
                remainder = rest;
            },
            Awaiting::EventName => {
                let (event_name, rest) = try_parse!(read_event_name(
                    remainder, expected_event_name_len.unwrap()));
                partial_message.event_name(event_name);
                consumed += remainder.len() - rest.len();
                remainder = rest;
                if current_message_type.unwrap().expects_payload() {
                    awaiting = Awaiting::PayloadLen;
                }
                else {
                    return ParseResult::Completed(partial_message.build_header().unwrap(),
                                                  consumed, 0);
                }
            },
            Awaiting::PayloadLen => {
                let (len, rest) = try_parse!(read_u16(remainder)
                                             .ok_or(ParseResult::Incomplete));
                consumed += remainder.len() - rest.len();
                return ParseResult::Completed(partial_message.build_header().unwrap(),
                                              consumed, len as usize);
            }
        };
    }
    ParseResult::Incomplete
}

fn read_message_type<'b>(b: &'b [u8])
                         -> Result<(MessageType, &'b [u8]), ParserError> {
    match read_u8(b) {
        Some((val, remainder)) => {
            let message_type = try!(match_message_type(val));
            Ok((message_type, remainder))
        },
        None => Err(ParserError::InsufficientData)
    }
}

fn match_message_type(val: u8) -> Result<MessageType, ParserError> {
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

fn read_event_name<'b>(b: &'b [u8], event_name_len: u8)
                       -> Result<(String, &'b [u8]), ParserError> {
    let (event_name_bytes, rest) = try!(take_n(event_name_len as usize, b)
                                        .ok_or(ParserError::InsufficientData));
    let event_name = try!(str::from_utf8(event_name_bytes)
                          .or(Err(ParserError::InvalidValue))).to_string();
    Ok((event_name, rest))

}


#[cfg(test)]
mod test {
    use super::*;
    use super::match_message_type;
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
        let expected_pairs = vec![(1, MessageType::Subscribe),
                                  (2, MessageType::Unsubscribe),
                                  (3, MessageType::Publish),
                                  (4, MessageType::Event)];
        for (val, message_type) in expected_pairs {
            assert_eq!(match_message_type(val), Ok(message_type));
        }
        assert_eq!(match_message_type(5), Err(ParserError::InvalidValue));
    }

    fn test_parse_message_without_payload(bytes: &[u8], expected_type: MessageType, expected_event_name: String,
                                          expected_consumed: usize) {
        let res = parse(&bytes);
        if let ParseResult::Completed(message_header, consumed, payload_len) = res {
            assert_eq!(consumed, expected_consumed);
            assert_eq!(payload_len, 0);
            assert_eq!(message_header.event_name, expected_event_name);
            assert_eq!(message_header.message_type, expected_type);
        }
        else {
            assert!(false, "Result wasn't Completed");
        }

    }

    #[test]
    fn test_parse_incomplete_message() {
        let message_bytes = vec![
            0x01, // Type (Subscribe)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e // Name (even; missing one byte)
                ];
        let res = parse(&message_bytes);
        assert_eq!(res, ParseResult::Incomplete);
    }

    #[test]
    fn test_parse_complete_message_without_payload() {
        let message_bytes = vec![
            0x01, // Type (Subscribe)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name (event)
            ];
        test_parse_message_without_payload(&message_bytes, MessageType::Subscribe, "event".to_string(),
                                           7);
    }

    #[test]
    fn test_parse_complete_message_without_payload_with_overflow() {
        let message_bytes = vec![
            0x01, // Type (Subscribe)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name (event)
            0x10, 0x20 // Extra bytes
            ];
        test_parse_message_without_payload(&message_bytes, MessageType::Subscribe, "event".to_string(),
                                           7);
    }

    fn test_parse_message_with_payload(bytes: &[u8], expected_type: MessageType, expected_event_name: String,
                                       expected_payload: &[u8], expected_consumed: usize) {
        let res = parse(bytes);
        if let ParseResult::Completed(message_header, consumed, payload_len) = res {
            assert_eq!(consumed, expected_consumed);
            assert_eq!(payload_len as usize, expected_payload.len());

            let payload_bytes = &bytes[consumed..consumed + payload_len as usize];
            assert_eq!(payload_bytes, expected_payload);

            assert_eq!(message_header.message_type, expected_type);
            assert_eq!(message_header.event_name, expected_event_name);
        }
        else {
            assert!(false, "Result wasn't Completed");
        }
    }

    #[test]
    fn test_parse_complete_message_with_payload() {
        let message_bytes = vec![
            0x03, // Type (Publish)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name (event)
            0x00, 0x0E, // Payload length
            0x61, 0x20, 0x70, 0x61, 0x79,// Payload (a payload here)
            0x6c, 0x6f, 0x61, 0x64, 0x20,
            0x68, 0x65, 0x72, 0x65
                ];
        let expected_payload: Vec<u8> = vec![0x61, 0x20, 0x70, 0x61, 0x79,
                                             0x6c, 0x6f, 0x61, 0x64, 0x20,
                                             0x68, 0x65, 0x72, 0x65];
        test_parse_message_with_payload(&message_bytes, MessageType::Publish, "event".to_string(),
                                        &expected_payload, 9);
    }

    #[test]
    fn test_parse_complete_message_with_payload_with_overflow() {
        let message_bytes = vec![
            0x03, // Type (Publish)
            0x05, // Name length
            0x65, 0x76, 0x65, 0x6e, 0x74, // Name (event)
            0x00, 0x0E, // Payload length
            0x61, 0x20, 0x70, 0x61, 0x79,// Payload (a payload here)
            0x6c, 0x6f, 0x61, 0x64, 0x20,
            0x68, 0x65, 0x72, 0x65,
            0x01, 0xAA, 0xBB // Extra bytes
                ];
        let expected_payload: Vec<u8> = vec![0x61, 0x20, 0x70, 0x61, 0x79,
                                             0x6c, 0x6f, 0x61, 0x64, 0x20,
                                             0x68, 0x65, 0x72, 0x65];

        test_parse_message_with_payload(&message_bytes, MessageType::Publish, "event".to_string(),
                                        &expected_payload, 9);

    }
}
