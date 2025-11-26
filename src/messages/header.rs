
use crate::messages::*;

#[derive(PartialEq, Debug)]
pub struct MessageHeader {
    pub marker: [u8; 16], // always all bits set
    pub length: u16, // min 19 max 4096
    pub message_type: MessageType, // 1-4 open, update, notification, keepalive
}



pub fn build_message_header(message_type: MessageType) -> MessageHeader {
    let marker: [u8; 16] = [0xFF; 16];
    let length: u16 = match message_type {
        MessageType::Keepalive => 19,
        MessageType::Open => 29,
        MessageType::Update => 23,
        _ => 19, // default length for other types of messages
    };

    MessageHeader {
        marker,
        length,
        message_type
    }
}