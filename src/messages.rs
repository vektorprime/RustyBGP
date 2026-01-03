use tokio::net::TcpStream;

use std::fmt;

pub mod keepalive;
pub mod open;
pub mod header;
pub mod update;
pub(crate) mod route_refresh;
pub(crate) mod notification;

use keepalive::*;
use open::*;
use notification::*;

use update::*;
use crate::errors::{BGPError, MessageError};
use crate::messages::route_refresh::{handle_route_refresh_message, RouteRefreshMessage};
use crate::neighbors::Neighbor;
use crate::process::BGPProcess;
// pub enum Message {
//     Open,
//     Update,
//     Notification,
//     Keepalive
// }

#[derive(PartialEq, Debug, Clone)]
pub enum AddressFamily {
    IPv4,
    IPv6,
}


//
// #[derive(PartialEq, Debug, Clone)]
// pub enum MessageEvent {
//     // use these for the neighbor message queue
//     Open(OpenMessage),
//     Update(UpdateMessage),
//     Notification(NotificationMessage),
//     Keepalive,
//     RouteRefresh(RouteRefreshMessage)
// }

#[derive(PartialEq, Debug, Clone)]
pub enum MessageType {
    Open,
    Update,
    Notification,
    Keepalive,
    RouteRefresh
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MessageType::Open => write!(f, "Message type is Open"),
            MessageType::Update => write!(f, "Message type is Update"),
            MessageType::Notification => write!(f, "Message type is Notification"),
            MessageType::Keepalive => write!(f, "Message type is Keepalive"),
            MessageType::RouteRefresh => write!(f, "Message type is RouteRefresh")
        }

    }
}

impl MessageType {
    pub fn to_u8(&self) -> u8 {
        match self {
            MessageType::Open => 1,
            MessageType::Update => 2,
            MessageType::Notification => 3,
            MessageType::Keepalive => 4,
            MessageType::RouteRefresh => 5,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum BGPVersion {
    V4
}

impl fmt::Display for BGPVersion  {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BGPVersion::V4   => write!(f, "BGP Version is 4"),
        }

    }
}

impl BGPVersion {
    pub fn to_u8(&self) -> u8 {
        match self {
            BGPVersion::V4 => 4,
        }
    }

    pub fn from_u8(byte: u8) -> Self {
        match byte {
            4 => BGPVersion::V4,
            _ => {panic!("Only BGP Version 4 is supported")}
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct OptionalParameter {
    param_type: u8,
    param_length: u8,
    // TODO make the params structs or enums
    parameter_value: Vec<u8>, //variable length
}

pub fn parse_packet_type(tsbuf: &Vec<u8>) -> Result<MessageType, BGPError> {
    match tsbuf.get(0..16) {
        Some(val) => {
            if val == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF ] {
                match tsbuf.get(18) {
                    Some(1) => Ok(MessageType::Open),
                    Some(2) => Ok(MessageType::Update),
                    Some(3) => Ok(MessageType::Notification),
                    Some(4) => Ok(MessageType::Keepalive),
                    Some(5) => Ok(MessageType::RouteRefresh),
                    _ => Err(MessageError::UnknownMessageType.into())
                }
            }
            else {
                Err(MessageError::NoMarkerFound.into())
            }
        }
        None => Err(MessageError::BufferEmpty.into())
    }
}





pub fn locate_marker_in_message(tsbuf: &[u8], migrated_data_len: usize) -> Result<(), MessageError> {

    for i in migrated_data_len + 0..migrated_data_len + 16 {
        if let Some(b) = tsbuf.get(i) {
            if *b != 0xFF {
                Err(MessageError::NoMarkerFound)?;
            }
        }
    }
    //println!("Found marker in message");
    Ok(())
}

// pub fn locate_markers_indexes_in_message(tsbuf: &[u8]) -> Result<(Vec<usize>), MessageError> {
//     // find all the Markers (16x 0xFF) and note their starting indexes in the vec
//     let mut markers_found = Vec::new();
//     // can't init the last_found_idx to 0 or else the first iteration would fail
//     let mut last_found_idx = 4096;
//     let bytes_to_find: [u8; 16] = [ 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
//     for (i, b) in tsbuf.iter().enumerate() {
//         if *b == 0xFF  && i + 16 < tsbuf.len() {
//             if tsbuf[i..i + 16] == bytes_to_find {
//                 if i == last_found_idx + 1 {
//                     // there's no way we have another marker again as we need at least 3 bytes between them
//                     println!("skipping marker because there's a trailing 0xFF");
//                     continue;
//                 }
//                 println!("Found marker in message at byte {}", i);
//                 last_found_idx = i;
//                 markers_found.push(i);
//             }
//
//
//         }
//     }
//     if markers_found.len() == 0 {
//         return Err(MessageError::NoMarkerFound)
//     }
//
//     Ok(markers_found)
//     //println!("Found marker in message");
// }

pub fn extract_messages_from_rec_data(tsbuf: &[u8]) -> Result<Vec<Vec<u8>>, MessageError> {
    // multiple messages may be contained in one tcp stream
    // all the messages are separated by marker and len

    // validate first 16 bytes are 0xFF
    // split the buffer per message
    // get len and advance to next marker

    let mut messages: Vec<Vec<u8>> = Vec::new();
    // println!("Total message size from TCP is: {}", data_len);

    // I merged this function and locate_marker because I wasn't handling leading or trailing 0xFF correctly
    // I needed to incorporate the message_len into the indexing of the tsbuf to properly traverse it

    let bytes_to_find: [u8; 16] = [ 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut bytes_to_skip = 0;
    for (i, b) in tsbuf.iter().enumerate() {
        if bytes_to_skip > 0 {
            bytes_to_skip -= 1;
            continue;
        }
        // println!("i is {}", i);
        if *b == 0xFF  && i + 16 < tsbuf.len() {
            if tsbuf[i..i + 16] == bytes_to_find {

                println!("Found marker in message at byte {}", i);
                let message_len =  match tsbuf.get(i +16..i + 18) {
                    Some(bytes) => {
                        u16::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt16Read)?)
                    },
                    None => {
                        println!("Unable to read message_len, skipping all messages in this TCP payload");
                        return Err(MessageError::BadInt16Read)
                    }
                };
                // println!("message_len is {}", message_len);
                if message_len as usize + i > tsbuf.len() {
                    return Err(MessageError::MessageLenTooBig);
                }

                messages.push(Vec::from(&tsbuf[i..i + message_len as usize]));
                bytes_to_skip = message_len;
                if i + message_len as usize >= tsbuf.len() {
                    break;
                }
            }
        }
    }

    Ok(messages)
}




#[cfg(test)]
mod tests {
    use super::*;

    // mod markers {
    //     use super::*;
    //     #[test]
    //     fn test_locate_markers_indexes_in_message() {
    //         // tests finding markers in a multi-message payload
    //         let msg_bytes: Vec<u8> = vec![
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0x00, 0x39, 0x02, 0x00, 0x00, 0x00, 0x18, 0x40,
    //             0x01, 0x01, 0x00, 0x40, 0x02, 0x0a, 0x02, 0x02,
    //             0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
    //             0x40, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x03, 0x00,
    //             0x20, 0x01, 0x01, 0x01, 0x01, 0x18, 0x0a, 0x00,
    //             0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0x00, 0x17, 0x05, 0x00, 0x01, 0x02, 0x01,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0x00, 0x17, 0x02, 0x00, 0x00, 0x00, 0x00
    //         ];
    //         let markers_indexes = locate_markers_indexes_in_message(&msg_bytes).unwrap();
    //         assert_eq!(markers_indexes.len(), 3);
    //     }
    //
    //     #[test]
    //     fn test_locate_message_lens_from_indexes_in_message() {
    //         // tests finding the msg lens after the marker in a multi-message payload
    //         let msg_bytes: Vec<u8> = vec![
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0x00, 0x39, 0x02, 0x00, 0x00, 0x00, 0x18, 0x40,
    //             0x01, 0x01, 0x00, 0x40, 0x02, 0x0a, 0x02, 0x02,
    //             0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
    //             0x40, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x03, 0x00,
    //             0x20, 0x01, 0x01, 0x01, 0x01, 0x18, 0x0a, 0x00,
    //             0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0x00, 0x17, 0x05, 0x00, 0x01, 0x02, 0x01,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    //             0x00, 0x17, 0x02, 0x00, 0x00, 0x00, 0x00
    //         ];
    //         let markers_indexes = locate_markers_indexes_in_message(&msg_bytes).unwrap();
    //         let mut msg_lens: [u16; 3] = [0; 3];
    //         let mut msg_lens_idx = 0usize;
    //         for index in markers_indexes {
    //             let message_len =  match msg_bytes.get(index +16..index + 18) {
    //                 Some(bytes) => {
    //                     u16::from_be_bytes(bytes.try_into().unwrap())
    //                 },
    //                 None => { continue }
    //             };
    //             msg_lens[msg_lens_idx] = message_len;
    //             msg_lens_idx += 1;
    //         }
    //
    //         assert_eq!(msg_lens[0], 57);
    //         assert_eq!(msg_lens[1], 23);
    //         assert_eq!(msg_lens[2], 23);
    //
    //     }
    //
    //
    // }



    mod parse_packet_type {
        use super::*;

        #[test]
        fn test_parse_packet_type_open_message() {
            let msg_bytes: Vec<u8> = vec![
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0x00, 0x39, 0x01, 0x04, 0x00, 0x01, 0x00, 0xb4, 0xc0, 0xa8, 0xc8, 0x01, 0x1c, 0x02, 0x06, 0x01,
                0x04, 0x00, 0x01, 0x00, 0x01, 0x02, 0x02, 0x80, 0x00, 0x02, 0x02, 0x02, 0x00, 0x02, 0x02, 0x46,
                0x00, 0x02, 0x06, 0x41, 0x04, 0x00, 0x00, 0x00, 0x01
            ];
            assert_eq!(parse_packet_type(&msg_bytes).unwrap(), MessageType::Open);
        }

        #[test]
        fn test_parse_packet_type_keepalive_message() {
            let msg_bytes: Vec<u8> = vec![
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0x00, 0x13, 0x04
            ];
            assert_eq!(parse_packet_type(&msg_bytes).unwrap(), MessageType::Keepalive);
        }

        #[test]
        fn test_parse_packet_type_route_refresh_message() {
            let msg_bytes: Vec<u8> = vec![
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0x00, 0x17, 0x05, 0x00, 0x01, 0x01, 0x01
            ];
            assert_eq!(parse_packet_type(&msg_bytes).unwrap(), MessageType::RouteRefresh);
        }

        #[test]
        fn test_parse_packet_type_update_message() {
            let msg_bytes: Vec<u8> = vec![
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0x00, 0x34, 0x02, 0x00, 0x00, 0x00, 0x18, 0x40,
                0x01, 0x01, 0x00, 0x40, 0x02, 0x0a, 0x02, 0x02,
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03,
                0x40, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x17, 0x20,
                0xc0, 0xa8, 0xc8, 0x02
            ];
            assert_eq!(parse_packet_type(&msg_bytes).unwrap(), MessageType::Update);
        }

        #[test]
        fn test_parse_packet_type_notification_message() {
            let msg_bytes: Vec<u8> = vec![
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0x00, 0x16, 0x03, 0x03, 0x0a, 0x0a
            ];
            assert_eq!(parse_packet_type(&msg_bytes).unwrap(), MessageType::Notification);
        }
    }
    
}