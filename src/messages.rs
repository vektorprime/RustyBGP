use std::net::{ Ipv4Addr, TcpListener, TcpStream};
use std::io::{Read, Write};

pub mod keepalive;
pub mod open;
pub mod header;
pub mod update;
mod route_refresh;

use keepalive::*;
use open::*;
use header::*;
use update::*;
use crate::errors::MessageError;
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

pub fn parse_packet_type(tsbuf: &Vec<u8>) -> MessageType {
    // TODO gracefully exit without panic
    match tsbuf.get(0) {
        Some(val) => {
            match tsbuf.get(18) {
                Some(1) => MessageType::Open,
                Some(2) => MessageType::Update,
                Some(3) => MessageType::Notification,
                Some(4) => MessageType::Keepalive,
                Some(5) => MessageType::RouteRefresh,
                _ => panic!("Unknown MessageType in parse_packet_type")
            }
        }
        None => panic!("Unable to read first byte of buffer in parse_packet_type")
    }
}

pub fn locate_marker_in_message(tsbuf: &Vec<u8>, migrated_data_len: usize) -> Result<(), MessageError> {


    for i in migrated_data_len + 0..migrated_data_len + 16 {
        if let Some(b) = tsbuf.get(i) {
            if *b != 0xFF {
                Err(MessageError::NoMarkerFound)?;
            }
        }
    }

    Ok(())
}

pub fn extract_messages_from_rec_data(tsbuf: &Vec<u8>) -> Result<Vec<Vec<u8>>, MessageError> {
    // multiple messages may be contained in one tcp stream
    // all the messages are separated by marker and len
    //
    // validate first 16 bytes are 0xFF
    // split the buffer per message
    // get len and advance to next marker

    let mut messages: Vec<Vec<u8>> = Vec::new();

    let data_len = tsbuf.len();
    let migrated_data_len: usize = 0;
    println!("Total message size from TCP is: {}", data_len);
    loop {
        locate_marker_in_message(tsbuf, migrated_data_len)?;
        println!("Found marker in message");

        let message_len = u8::from_be_bytes(tsbuf[migrated_data_len + 16..migrated_data_len + 17].try_into().unwrap());
        println!("Found message length: {}", message_len);

        messages.push(Vec::from(&tsbuf[0..message_len as usize]));

        if migrated_data_len == data_len {
            break;
        }
    }



    Ok(messages)
}

pub fn route_incomming_message_to_handler(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) {
    let message_type = parse_packet_type(&tsbuf);
    println!("Message type is: {:?}", message_type);
    match message_type {
        MessageType::Open => {
            handle_open_message(tcp_stream, tsbuf);
        },
        MessageType::Update => {
          // handle_update_message(tcp_stream, tsbuf);
        },
        MessageType::Notification => {
            // TODO handle_notification_message
        },
        MessageType::Keepalive => {
            handle_keepalive_message(tcp_stream);
        },
        MessageType::RouteRefresh => {
            // TODO handle route refresh
            handle_route_refresh_message(tcp_stream);
        }
    }
}
