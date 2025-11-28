use std::net::{ Ipv4Addr, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::fmt;

pub mod keepalive;
pub mod open;
pub mod header;
pub mod update;
mod route_refresh;

use keepalive::*;
use open::*;
use header::*;
use update::*;
use crate::errors::{BGPError, MessageError};
use crate::messages::route_refresh::handle_route_refresh_message;
// pub enum Message {
//     Open,
//     Update,
//     Notification,
//     Keepalive
// }

#[derive(PartialEq, Debug)]
pub enum AddressFamily {
    IPv4,
    IPv6,
}



#[derive(PartialEq, Debug)]
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

#[derive(PartialEq, Debug)]
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
}

#[derive(PartialEq, Debug)]
pub struct OptionalParameter {
    param_type: u8,
    param_length: u8,
    // TODO make the params structs or enums
    parameter_value: Vec<u8>, //variable length
}

pub fn parse_packet_type(tsbuf: &Vec<u8>) -> Result<MessageType, MessageError> {
    // TODO gracefully exit without panic
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
                    _ => Err(MessageError::UnknownMessageType)
                }
            }
            else {
                Err(MessageError::NoMarkerFound)
            }

        }
        None => Err(MessageError::BufferEmpty)
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

pub fn locate_markers_indexes_in_message(tsbuf: &[u8]) -> Result<(Vec<usize>), MessageError> {
    // find all the Markers (16x 0xFF) and note their starting indexes in the vec
    let mut markers_found = Vec::new();
    let bytes_to_find: [u8; 16] = [ 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    for (i, b) in tsbuf.iter().enumerate() {
        if *b == 0xFF {
            if i + 16 <tsbuf.len() {
                if tsbuf[i..i + 16] == bytes_to_find {
                    markers_found.push(i);
                }
            }

        }
    }
    if markers_found.len() == 0 {
        return Err(MessageError::NoMarkerFound)
    }

    Ok(markers_found)
    //println!("Found marker in message");
}

pub fn extract_messages_from_rec_data(tsbuf: &[u8], data_len: usize) -> Result<Vec<Vec<u8>>, MessageError> {
    // multiple messages may be contained in one tcp stream
    // all the messages are separated by marker and len

    // validate first 16 bytes are 0xFF
    // split the buffer per message
    // get len and advance to next marker

    let mut messages: Vec<Vec<u8>> = Vec::new();
    //println!("Total message size from TCP is: {}", data_len);
    loop {
        //locate_marker_in_message(tsbuf, migrated_data_len)?;
        let markers_indexes = locate_markers_indexes_in_message(tsbuf)?;
        for index in markers_indexes {
            let message_len =  match tsbuf.get(index +16..index + 18) {
                Some(bytes) => {
                    u16::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadIntRead)?)
                },
                None => { continue }
            };
            println!("Found message length: {}", message_len);

            messages.push(Vec::from(&tsbuf[index..index + message_len as usize]));
        }
        break;
    }
    
    Ok(messages)
}

pub fn route_incomming_message_to_handler(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) -> Result<(), BGPError> {
    let message_type = parse_packet_type(&tsbuf)?;
    println!("{}", message_type);
    match message_type {
        MessageType::Open => {
            handle_open_message(tcp_stream, tsbuf)?
        },
        MessageType::Update => {
            // handle_update_message(tcp_stream, tsbuf);
        },
        MessageType::Notification => {
            // TODO handle_notification_message
        },
        MessageType::Keepalive => {
            handle_keepalive_message(tcp_stream)?
        },
        MessageType::RouteRefresh => {
            // TODO handle route refresh
            handle_route_refresh_message(tcp_stream, tsbuf)?
        }

    }
    Ok(())
}
