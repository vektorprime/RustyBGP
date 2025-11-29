use std::net::{ Ipv4Addr, IpAddr, TcpListener, TcpStream};
use std::io::{Read, Write};

use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
use crate::routes::RouteV4;
use crate::utils::{extract_u16_from_bytes, extract_u32_from_bytes, extract_u8_from_byte};

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
pub struct Flags {
    optional: Flag,
    transitive: Flag,
    partial: Flag,
    extended_length: Flag,
}


#[derive(PartialEq, Debug)]
pub enum Flag {
    Optional(bool), // bit 0
    Transitive(bool), // bit 1
    Partial(bool), // bit 2
    // TODO handle extended length causing the attribute length field to be 2 bytes
    ExtendedLength(bool), // bit 3
}

impl Flags {
    pub fn new() -> Self {
        Flags {
            optional: Flag::Optional(false),
            transitive: Flag::Transitive(false),
            partial: Flag::Partial(false),
            extended_length: Flag::ExtendedLength(false),
        }
    }
    pub fn from_u8(val: u8) -> Self {
        let mut flags = Flags::new();
        if 0b1000_0000 & val == 0b1000_0000 {
            flags.optional = Flag::Optional(true);
        }
        if 0b0100_0000 & val == 0b0100_0000 {
            flags.transitive = Flag::Transitive(true);
        }
        if 0b0010_0000 & val == 0b0010_0000 {
            flags.partial = Flag::Partial(true);
        }
        if 0b0001_0000 & val == 0b0001_0000 {
            flags.extended_length = Flag::ExtendedLength(true);
        }
        flags
    }
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

impl TypeCode {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => TypeCode::Origin,
            1 => TypeCode::AsPath,
            2 => TypeCode::NextHop,
            3 => TypeCode::MultiExitDisc,
            4 => TypeCode::LocalPref,
            5 => TypeCode::AtomicAggregate,
            6 => TypeCode::Aggregator,
            _ => unreachable!()
        }
    }
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

impl Origin {
    pub fn from_u8(val: u8) -> Self {
        let origin_type = match val {
            0 => OriginType::IGP,
            1 => OriginType::EGP,
            2 => OriginType::Incomplete,
            _ => panic!("Unknown origin type in Oirigin::from_u8")
        };
        Origin {
            category: Category::WellKnownMandatory,
            origin_type
        }
    }
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

impl AsPathSegmentType {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => AsPathSegmentType::ASSet,
            2 => AsPathSegmentType::AsSequence,
            _ => panic!("Unknown AsPathSegmentType in AsPathSegmentType::from_u8")
        }
    }
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
    pub as_list: Vec<AS> // 2 or 4 bytes each
}

impl AsPath {
    pub fn from_vec_u8(bytes: &Vec<u8>) -> Self {
        let segment_type = AsPathSegmentType::from_u8(bytes[0]);

        let number_of_as: u8 = bytes[1];
        let as_list = {
            let mut as_list: Vec<AS> = Vec::new();
            let base_idx: usize = 2;
            for i in 0..number_of_as as usize {
                let current_index = base_idx + (i * 4);
                let as_num_bytes = &bytes[current_index.. current_index + 4];
                as_list.push(AS::AS4(u32::from_be_bytes(as_num_bytes.try_into().unwrap())));
            }
            as_list
        };
        let as_path_segment = {
            AsPathSegment {
                segment_type,
                number_of_as,
                as_list
            }


        };
        AsPath {
            category: Category::WellKnownMandatory,
            as_path_segment
        }
    }
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

impl Data {
    pub fn from_vec_u8(type_code: &TypeCode, bytes: &Vec<u8>) -> Self{
        match type_code {
            TypeCode::Origin => {
                Data::Origin(Origin::from_u8(bytes[0]))
            },
            TypeCode::AsPath => {
                Data::AsPath(AsPath::from_vec_u8(bytes))
            },
        }
    }
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
    println!("extracting update message");
    let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;
    println!("message_len: {}", message_len);
    let withdrawn_route_len = extract_u16_from_bytes(tsbuf, 19, 21)?;
    println!("withdrawn route len is : {}", withdrawn_route_len);

    let base_idx = 21;
    let route_size = 5;
    let withdrawn_routes: Option<Vec<NLRI>> = if withdrawn_route_len >= route_size {

        let mut routes: Vec<NLRI> = Vec::new();
        for x in 0.. (withdrawn_route_len / route_size) as usize {
            let mut current_idx = base_idx + (route_size as usize * x);
            let prefix_len = extract_u8_from_byte(tsbuf, current_idx, current_idx + 1)?;
            current_idx += 1;
            let route_u32 = extract_u32_from_bytes(tsbuf, current_idx, current_idx + route_size as usize)?;
            routes.push(NLRI{
                len: prefix_len,
                prefix: route_u32
            });
        }
        Some(routes)
    } else {
        None
    };
    let mut pa_idx = base_idx + withdrawn_route_len as usize;
    let total_path_attribute_len = extract_u16_from_bytes(tsbuf, pa_idx, pa_idx + 2)?;
    pa_idx += 2;
    // origin, aspath, next hop are the mandatory atts (24 total).
    let path_attributes: Option<Vec<PathAttribute>> = if total_path_attribute_len >= 24 {
        let flags = {
            let val = extract_u8_from_byte(tsbuf, pa_idx, pa_idx + 1)?;
            Flags::from_u8(val)
        };
        pa_idx += 1;
        let type_code =  {
            let val =  extract_u8_from_byte(tsbuf, pa_idx, pa_idx + 1)?;
            TypeCode::from_u8(val)
        };
        pa_idx += 1;
        let len = extract_u8_from_byte(tsbuf, pa_idx, pa_idx + 1)?;
        pa_idx += 1;
        let data = {
            let mut bytes: Vec<u8> = Vec::new();
            for x in 0..len as usize {
                match tsbuf.get(pa_idx + x) {
                    Some(byte) => { bytes.push(*byte); },
                    None => { return Err(MessageError::InvalidBufferIndex) }
                }
            }
            Data::from_vec_u8(&type_code, &bytes)

        };

        PathAttribute {
            flags,
            type_code,
            len,
            data
        }
    } else {
        None
    };

    /////

    // TODO read and process the optional params
    let update_message = UpdateMessage::new(message_len, withdrawn_route_len, withdrawn_routes, total_path_attribute_len, path_attributes );

    Ok(update_message)

}

impl UpdateMessage {
    pub fn new(message_len: u16, withdrawn_route_len: u16, withdrawn_routes: Option<Vec<NLRI>>, total_path_attribute_len: u16, path_attributes: Option<Vec<PathAttribute>> ) -> Self {
        let message_header = MessageHeader::new(MessageType::Update, Some(message_len));
        UpdateMessage {
            message_header,
            withdrawn_route_len,
            withdrawn_routes,
            total_path_attribute_len,
            path_attributes,
        }
    }
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
