use std::net::{ Ipv4Addr };
use tokio::net::{TcpStream, TcpListener};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use crate::messages::header::*;
use crate::messages::*;

// pub async fn handle_keepalive_message(tcp_stream: &mut OwnedWriteHalf) -> Result<(), MessageError> {
//     send_keepalive(tcp_stream).await?;
//     Ok(())
// }

pub async fn send_keepalive(stream: &mut OwnedWriteHalf) -> Result<(), MessageError> {
    //TODO add peer as input var and match against DB
    //TODO add periodic keepalives
    println!("Preparing to send Keepalive");
    let message = KeepaliveMessage::new()?;
    let message_bytes = message.convert_to_bytes();
    let ts = stream.write_all(&message_bytes[..]).await;
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
    pub fn new() -> Result<Self, MessageError> {
        let message_header = MessageHeader::new(MessageType::Keepalive, None)?;
       Ok(KeepaliveMessage {
           message_header
       })
    }

    pub fn convert_to_bytes(&self) -> Vec<u8> {

        let mut message: Vec<u8> = vec![0xFF; 16];

        let mut len: u16 = message.len() as u16;
        len += 2;

        let message_type = MessageType::Keepalive;

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