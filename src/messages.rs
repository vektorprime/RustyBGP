use std::net::Ipv4Addr;

// pub enum Message {
//     Open,
//     Update,
//     Notification,
//     Keepalive
// }

#[derive(PartialEq, Debug)]
pub enum MessageType {
    Open,
    Update,
    Notification,
    Keepalive
}

impl MessageType {
    pub fn to_u8(&self) -> u8 {
        match self {
            MessageType::Open => 1,
            MessageType::Update => 2,
            MessageType::Notification => 3,
            MessageType::Keepalive => 4,
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct MessageHeader {
    pub marker: [u8; 16], // always all bits set
    pub length: u16, // min 19 max 4096
    pub message_type: MessageType, // 1-4 open, update, notification, keepalive
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


pub fn build_message_header(message_type: MessageType) -> MessageHeader {
    let marker: [u8; 16] = [0xFF; 16];
    let length: u16 = match message_type {
        MessageType::Keepalive => 19,
        MessageType::Open => 29,
        _ => 19, // default length for other types of messages
    };

    MessageHeader {
        marker,
        length,
        message_type
    }
}



#[derive(PartialEq, Debug)]
pub struct OpenMessage {
    // min length 29 bytes
    pub message_header: MessageHeader,
    pub version: BGPVersion, // only V4 is in use
    pub as_number: u16,
    pub hold_time: u16, //either 0 or at least 3 sec, lowest between the two peers
    pub identifier: Ipv4Addr,
    pub optional_parameters_length: u8,
    pub optional_parameters: Option<Vec<OptionalParameter>>,
}


impl OpenMessage {

    pub fn convert_to_bytes(&self) {
        let message_header = build_message_header(MessageType::Open);
        let mut message: Vec<u8> = Vec::new();
        // marker
        for b in message_header.marker {
            message.push(b);
        }
        let mut len: u16 = message.len() as u16;
        len += 2;

        let msg_type: u8 = message_header.message_type.to_u8();
        len += 1;

        let ver = self.version.to_u8();
        len += 1;

        let as_num_bytes = self.as_number.to_le_bytes();
        len += 2;

        let hold_time_bytes = self.hold_time.to_le_bytes();
        len += 2;

        let identifier_bytes = self.identifier.to_bits().to_le_bytes();
        len += 4;

        //len
        let len_bytes: [u8; 2] = len.to_le_bytes();
        message.push(len_bytes[0]); // len lo
        message.push(len_bytes[1]); // len hi

        // type
        message.push(msg_type); 

        // ver
        message.push(ver);

        // as_num
        message.push(as_num_bytes[0]);
        message.push(as_num_bytes[1]);


        // hold_time
        message.push(hold_time_bytes[0]);
        message.push(hold_time_bytes[1]);

        // identifier
        message.push(identifier_bytes[0]);
        message.push(identifier_bytes[1]);
        message.push(identifier_bytes[2]);
        message.push(identifier_bytes[3]);

        //opt len testing as 0
        message.push(0);

    }

}
