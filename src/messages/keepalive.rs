use std::net::{ Ipv4Addr, TcpListener, TcpStream};
use std::io::{Read, Write};

use crate::messages::header::*;
use crate::messages::*;

pub fn handle_keepalive_message(tcp_stream: &mut TcpStream) -> Result<(), MessageError> {
    send_keepalive(tcp_stream)?;
    Ok(())
}

pub fn send_keepalive(stream: &mut TcpStream) -> Result<(), MessageError> {
    //TODO add peer as input var and match against DB
    //TODO add periodic keepalives
    println!("Preparing to send Keepalive");
    let message = KeepaliveMessage::new();
    let message_bytes = message.convert_to_bytes();
    let ts = stream.write_all(&message_bytes[..]);
    match ts {
        Ok(_) => {
            println!("Sent Keepalive");
            Ok(())
        },
        Err(_) => {
            Err(MessageError::UnableToWriteToTCPStream)
        }
    }

}

#[derive(PartialEq, Debug)]
pub struct KeepaliveMessage {
    pub message_header: MessageHeader,
}

impl KeepaliveMessage {
    pub fn new() -> Self {
        let message_header = build_message_header(MessageType::Keepalive);
        KeepaliveMessage {
            message_header
        }
    }

    pub fn convert_to_bytes(&self) -> Vec<u8> {
        let mut message: Vec<u8> = Vec::new();

        let message_header = build_message_header(MessageType::Keepalive);

        for b in message_header.marker {
            message.push(b);
        }

        let mut len: u16 = message.len() as u16;
        len += 2;

        let msg_type: u8 = message_header.message_type.to_u8();
        len += 1;

        // adding len to the vec must come second to last because we need the total len of the payload
        let len_bytes: [u8; 2] = len.to_be_bytes();
        message.push(len_bytes[0]);
        message.push(len_bytes[1]);

        message.push(msg_type);

        message
    }
}