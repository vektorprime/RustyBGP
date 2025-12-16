use std::net::{ Ipv4Addr};
use tokio::net::{TcpStream, TcpListener};
//use std::io::{Read, Write};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

use crate::messages::header::*;
use crate::messages::*;
use crate::utils::extract_u16_from_bytes;

pub fn handle_route_refresh_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) -> Result<(), MessageError> {
    //send_route_refresh(tcp_stream);
    println!("Handling route refresh message");
    // TODO handle route refresh
    match tsbuf.get(19..21) {
        Some(ts) => {
            let afi = extract_u16_from_bytes(tsbuf, 0, 2)?;
            if afi == 1 {
                println!("Received route refresh for IPv4 AFI");
            }
            else if afi == 1 {
                println!("Received route refresh for IPv4 AFI");
            }
        },
        None => {
            println!("No AFI found in route refresh message");
            return Err(MessageError::RouteRefreshMissingAFI)
        }
    }
    // readv the adj rib out for the afi, safi pair
    println!("Handled route refresh message");
    Ok(())
}

pub async fn send_route_refresh(stream: &mut TcpStream, afi: AddressFamily, safi: SAFI) -> Result<(), MessageError> {
    // TODO
    println!("Preparing to send RouteRefresh");
    let message = RouteRefreshMessage::new(afi, safi)?;
    let message_bytes = message.convert_to_bytes();
    stream.write_all(&message_bytes[..]).await.unwrap();
    println!("Sent RouteRefresh");
    Ok(())
}

#[derive(PartialEq, Debug)]
pub enum SAFI {
    Unicast,
    Multicast
}

#[derive(PartialEq, Debug)]
pub struct RouteRefreshMessage {
    pub message_header: MessageHeader,
    pub afi: AddressFamily, // 2 bytes
    // reserved 1 byte
    pub safi: SAFI, // 1 byte


}

impl RouteRefreshMessage {
    pub fn new(afi: AddressFamily, safi: SAFI) -> Result<Self, MessageError> {
        // Route refresh is always 23 bytes
        let message_header = MessageHeader::new(MessageType::RouteRefresh, Some(23))?;
       Ok(RouteRefreshMessage {
           message_header,
           afi,
           safi
       })
    }

    pub fn convert_to_bytes(&self) -> Vec<u8> {
        let mut message: Vec<u8> = vec![0xFF; 16];

        let mut len: u16 = message.len() as u16;
        len += 2;

        let message_type = MessageType::RouteRefresh;

        let msg_type: u8 = message_type.to_u8();
        len += 1;

        // adding len to the vec must come second to last because we need the total len of the payload
        let len_bytes: [u8; 2] = len.to_be_bytes();
        message.push(len_bytes[0]);
        message.push(len_bytes[1]);

        message.push(msg_type);

        message
    }
}