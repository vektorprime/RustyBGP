
use crate::messages::*;

#[derive(PartialEq, Debug, Clone)]
pub struct MessageHeader {
    pub marker: [u8; 16], // always all bits set
    pub length: u16, // min 19 max 4096
    pub message_type: MessageType, // 1-4 open, update, notification, keepalive
}

impl fmt::Display for MessageHeader  {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
       write!(f, "MessageHeader marker is 16 bytes of {:?},
        length is {}, MessageType is {}", self.marker, self.length, self.message_type)
    }
}

impl MessageHeader {
    pub fn new(message_type: MessageType, len: Option<u16>) -> Result<Self, MessageError> {
        let marker: [u8; 16] = [0xFF; 16];
        let length = match len {
            Some(l) => {
                if l >= 19 && l <= 4096 {
                    l
                }
                else {
                    return Err(MessageError::MessageHeaderBadLen)
                }
            },
            None => {
                match message_type {
                    MessageType::Keepalive => 19,
                    MessageType::Open => 29,
                    MessageType::Update => 23,
                    _ => 19, // default length for other types of messages
                }
            }
        };
        Ok(MessageHeader {
            marker,
            length,
            message_type
        })

    }
}
