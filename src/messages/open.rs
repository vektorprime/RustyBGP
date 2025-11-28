use std::net::{ Ipv4Addr, TcpListener, TcpStream};
use std::io::{Read, Write};

use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
pub fn handle_open_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) -> Result<(), MessageError> {
    //TODO handle better, for now just accept the neighbor and mirror the capabilities for testing
    let open_message = OpenMessage::new(2, 180, Ipv4Addr::new(1,1,1,1), 0, None);
    send_open(tcp_stream, open_message)?;
    send_keepalive(tcp_stream)?;
    Ok(())
}

pub fn send_open(stream: &mut TcpStream, message: OpenMessage) -> Result<(), MessageError> {
    println!("Preparing to send Open");
    let message_bytes = message.convert_to_bytes();
    let res  =stream.write_all(&message_bytes[..]);
    match res {
        Ok(_) => {
            println!("Sent Open");
            Ok(())
        },
        Err(_) => {
            Err(MessageError::UnableToWriteToTCPStream)
        }
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
