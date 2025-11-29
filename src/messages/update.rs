use std::net::{ Ipv4Addr, IpAddr, TcpListener, TcpStream};
use std::io::{Read, Write};

use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
use crate::utils::{extract_u16_from_bytes, extract_u32_from_bytes};

pub fn handle_update_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) {
    println!("handling update message");
    
}

pub fn send_update(stream: &mut TcpStream, message: UpdateMessage) {
    println!("Preparing to send update");
   // let message_bytes = message.convert_to_bytes();
    //stream.write_all(&message_bytes[..]).unwrap();
    println!("Sent update");
}

#[derive(PartialEq, Debug)]
pub struct NLRI {
    len: u8,
    // TODO handle bigger prefixes and padding/trailing bits so that this falls on a byte boundary
    prefix: u32
}

// #[derive(PartialEq, Debug)]
// pub enum PathAttributes {
//     Origin,
//     ASPath,
//     ASSequence,
//     NextHop,
//     MultiExitDisc,
// }

#[derive(PartialEq, Debug)]
pub enum Flags {
    Optional(bool), // bit 0
    Transitive(bool), // bit 1
    Partial(bool), // bit 2
    // TODO handle extended length causing the attribute length field to be 2 bytes
    ExtendedLength(bool), // bit 3
}

#[derive(PartialEq, Debug)]
pub enum TypeCode {
    Origin,
    AsPath,
    NextHop,
    MultiExitDisc,
    LocalPref,
    AtomicAggregate,
    Aggregator
}

#[derive(PartialEq, Debug)]
pub enum Category {
    WellKnownMandatory,
    WellKnownDiscretionary,
    OptionalTransitive,
    OptionalNonTransitive,
}
#[derive(PartialEq, Debug)]
pub struct Origin {
    category: Category,
    origin_type: OriginType
}

#[derive(PartialEq, Debug)]
pub enum OriginType {
    IGP,
    EGP,
    Incomplete
}

#[derive(PartialEq, Debug)]
pub enum AsPathSegmentType {
    ASSet,
    AsSequence
}

#[derive(PartialEq, Debug)]
pub enum AS {
    AS2(u16),
    AS4(u32)
}

#[derive(PartialEq, Debug)]
pub struct AsPath {
    category: Category,
    as_path_segment: AsPathSegment
}
#[derive(PartialEq, Debug)]
pub struct AsPathSegment {
    pub segment_type: AsPathSegmentType, // 1 byte
    pub number_of_as: u8, // number of ASes, not number of bytes
    // TODO handle 2 vs 4 byte AS
    pub as4: Vec<AS> // 2 or 4 bytes each
}

#[derive(PartialEq, Debug)]
pub struct NextHop {
    category: Category,
    // TODO handle other sizes
    ipv4addr: Ipv4Addr,
}

#[derive(PartialEq, Debug)]
pub struct MultiExitDisc {
    category: Category,
    value: u32, // 4 bytes
}

#[derive(PartialEq, Debug)]
pub struct LocalPref {
    category: Category,
    value: u32, // 4 bytes
}

#[derive(PartialEq, Debug)]
pub struct AtomicAggregate {
    category: Category,
    value: u32, // 4 bytes
}

#[derive(PartialEq, Debug)]
pub struct Aggregator {
    category: Category,
    as_num: AS, // 2 bytes or 4
    ipv4addr: Ipv4Addr
}


#[derive(PartialEq, Debug)]
pub enum Data {
    Origin(Origin),
    AsPath(AsPath),
    NextHop(NextHop),
    MultiExitDisc(MultiExitDisc),
    LocalPref(LocalPref),
    AtomicAggregate(AtomicAggregate),
    Aggregator(Aggregator)
}

#[derive(PartialEq, Debug)]
pub struct PathAttribute {
    flags: Flags,
    type_code: TypeCode,
    len: u8,
    data: Vec<Data>
}

#[derive(PartialEq, Debug)]
pub struct UpdateMessage {
    // min len is 23 bytes
    pub message_header: MessageHeader,
    pub withdrawn_route_len: u16, // size in bytes, not the number of objects
    pub withdrawn_routes: Option<Vec<NLRI>>,
    pub total_path_attribute_len: u16, // size in bytes, not the number of objects
    pub path_attributes: Option<Vec<PathAttribute>>
}

pub fn extract_update_message(tsbuf: &Vec<u8>) -> Result<UpdateMessage, MessageError> {
    // TODO I only copied this, I have not modified it yet
    let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;

    let withdrawn_route_len = extract_u16_from_bytes(tsbuf, 19, 21)?;

    let withdrawn_routes = if withdrawn_route_len != 0 {
        
    } else {
        None
    };

    let message_header = MessageHeader::new(MessageType::Update, Some(message_len));

    let as_number = extract_u16_from_bytes(tsbuf, 20, 22)?;

    let hold_time = extract_u16_from_bytes(tsbuf, 22, 24)?;

    let identifier = Ipv4Addr::from_bits(extract_u32_from_bytes(tsbuf, 30, 34)?);

    // TODO read and process the optional params
    let update_message = UpdateMessage::new();

    Ok(update_message)

}

impl UpdateMessage {
    // pub fn new(as_number: u16, hold_time: u16, identifier: Ipv4Addr, optional_parameters_length: u8, optional_parameters: Option<Vec<OptionalParameter>> ) -> Self {
    //     let message_header = build_message_header(MessageType::Update);
    //     UpdateMessage {
    //         message_header,
    //
    //     }
    // }
    // pub fn convert_to_bytes(&self) -> Vec<u8> {
    //     let message_header = build_message_header(MessageType::Update);
    //     let mut message: Vec<u8> = Vec::new();
    //     // marker
    //     for b in message_header.marker {
    //         message.push(b);
    //     }
    //     let mut len: u16 = message.len() as u16;
    //     len += 2;
    //
    //     let msg_type: u8 = message_header.message_type.to_u8();
    //     len += 1;
    //
    //     let ver = self.version.to_u8();
    //     len += 1;
    //
    //     let as_num_bytes = self.as_number.to_be_bytes();
    //     len += 2;
    //
    //     let hold_time_bytes = self.hold_time.to_be_bytes();
    //     len += 2;
    //
    //     let identifier_bytes = self.identifier.to_bits().to_be_bytes();
    //     len += 4;
    //
    //     let opt_params_len : u8 = 28;
    //     len += 1; // for the len field itself
    //     len += opt_params_len as u16; // for the params
    //
    //     // adding len to the vec must come second to last because we need the total len of the payload
    //     let len_bytes: [u8; 2] = len.to_be_bytes();
    //     message.push(len_bytes[0]);
    //     message.push(len_bytes[1]);
    //
    //     message.push(msg_type);
    //
    //     message.push(ver);
    //
    //     message.push(as_num_bytes[0]);
    //     message.push(as_num_bytes[1]);
    //
    //
    //     message.push(hold_time_bytes[0]);
    //     message.push(hold_time_bytes[1]);
    //
    //     message.push(identifier_bytes[0]);
    //     message.push(identifier_bytes[1]);
    //     message.push(identifier_bytes[2]);
    //     message.push(identifier_bytes[3]);
    //
    //     // TODO remove this after testing
    //     // sending len 28 and opt params taken from CSR packet cap
    //     //message.push(0);
    //     message.push(opt_params_len);
    //
    //     let opt_params: [u8; 28] = [0x02, 0x06, 0x01, 0x04, 0x00, 0x01, 0x00, 0x01, 0x02, 0x02, 0x80, 0x00, 0x02, 0x02, 0x02, 0x00, 0x02, 0x02, 0x46, 0x00, 0x02, 0x06, 0x41, 0x04, 0x00, 0x00, 0x00, 0x01];
    //     message.extend_from_slice(&opt_params);
    //
    //     message
    // }
}
