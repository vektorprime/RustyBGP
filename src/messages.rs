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

#[derive(PartialEq, Debug)]
pub struct OpenMessage {
    // min length 29 bytes
    pub message_header: MessageHeader,
    pub version: BGPVersion, // only V4 is in use
    pub as_number: u16,
    pub hold_time: u16, // either 0 or at least 3 sec, lowest between the two peers
    pub identifier: Ipv4Addr,
    pub optional_parameters_length: u8,
    pub optional_parameters: Option<Vec<OptionalParameter>>,
}


impl OpenMessage {
pub fn new(as_number: u16, hold_time: u16, identifier: Ipv4Addr, optional_parameters_length: u8, optional_parameters: Option<Vec<OptionalParameter>> ) -> Self {
    let message_header = build_message_header(MessageType::Open);
    OpenMessage {
        message_header,
        version: BGPVersion::V4,
        as_number,
        hold_time,
        identifier,
        optional_parameters_length,
        optional_parameters
    }
}
    pub fn convert_to_bytes(&self) -> Vec<u8> {
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

        let as_num_bytes = self.as_number.to_be_bytes();
        len += 2;

        let hold_time_bytes = self.hold_time.to_be_bytes();
        len += 2;

        let identifier_bytes = self.identifier.to_bits().to_be_bytes();
        len += 4;

        let opt_params_len : u8 = 28;
        len += 1; // for the len field itself
        len += opt_params_len as u16; // for the params

        // adding len to the vec must come second to last because we need the total len of the payload
        let len_bytes: [u8; 2] = len.to_be_bytes();
        message.push(len_bytes[0]);
        message.push(len_bytes[1]);

        message.push(msg_type); 

        message.push(ver);

        message.push(as_num_bytes[0]);
        message.push(as_num_bytes[1]);


        message.push(hold_time_bytes[0]);
        message.push(hold_time_bytes[1]);

        message.push(identifier_bytes[0]);
        message.push(identifier_bytes[1]);
        message.push(identifier_bytes[2]);
        message.push(identifier_bytes[3]);

        // TODO remove this after testing
        // sending len 28 and opt params taken from CSR packet cap
        //message.push(0);
        message.push(opt_params_len);

        let opt_params: [u8; 28] = [0x02, 0x06, 0x01, 0x04, 0x00, 0x01, 0x00, 0x01, 0x02, 0x02, 0x80, 0x00, 0x02, 0x02, 0x02, 0x00, 0x02, 0x02, 0x46, 0x00, 0x02, 0x06, 0x41, 0x04, 0x00, 0x00, 0x00, 0x01];
        message.extend_from_slice(&opt_params);

        message
    }

}
