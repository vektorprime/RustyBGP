use std::net::{ Ipv4Addr, IpAddr};
use tokio::net::{TcpStream, TcpListener};


use std::io::{Bytes, Read, Write};
use std::path::Path;
use std::thread::current;
use crate::errors::{NeighborError, ProcessError};
use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
use crate::neighbors::Neighbor;
use crate::routes::*;
use crate::utils::{extract_u16_from_bytes, extract_u32_from_bytes, extract_u8_from_byte};



// pub fn validate_neighbor_is_established(ts: &TcpStream, bgp_proc: &BGPProcess) -> Result<Ipv4Addr, NeighborError> {
//     match ts.peer_addr().unwrap().ip() {
//         IpAddr::V4(ip) => {
//             bgp_proc.neighbors.get(&ip).ok_or_else(|| NeighborError::PeerIPNotEstablished)?;
//             println!("Validated neighbor is established");
//             return Ok(ip);
//
//         },
//         _ => {
//             Err(NeighborError::PeerIPNotEstablished)
//         }
//     }
// }


#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Flag {
    Optional(bool), // bit 0
    Transitive(bool), // bit 1
    Partial(bool), // bit 2
    // TODO handle extended length causing the attribute length field to be 2 bytes
    ExtendedLength(bool), // bit 3
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Flags {
    optional: Flag,
    transitive: Flag,
    partial: Flag,
    extended_length: Flag,
}

// impl Default for Flags {
//     fn default() -> Self {
//         Flags {
//             optional: Flag::Optional(false),
//             transitive: Flag::Transitive(false),
//             partial: Flag::Partial(false),
//             extended_length: Flag::ExtendedLength(false),
//         }
//     }
// }


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

    pub fn to_u8(&self) -> u8 {
        let mut flags: u8 = 0;


        if let Flag::Optional(true) = self.optional {
            flags |= 0b1000_0000;
        }

        if let Flag::Transitive(true) = self.transitive {
            flags |= 0b0100_0000;
        }

        if let Flag::Partial(true) = self.partial {
            flags |= 0b0010_0000;
        }

        if let Flag::ExtendedLength(true) = self.extended_length {
            flags |= 0b0001_0000;
        }

        flags
    }

}

#[derive(PartialEq, Debug, Copy, Clone)]
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
            // 0 is reserved
            1 => TypeCode::Origin,
            2 => TypeCode::AsPath,
            3 => TypeCode::NextHop,
            4 => TypeCode::MultiExitDisc,
            5 => TypeCode::LocalPref,
            6 => TypeCode::AtomicAggregate,
            7 => TypeCode::Aggregator,
            _ => unreachable!()
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            // 0 is reserved
            TypeCode::Origin           =>   1,
            TypeCode::AsPath           =>   2,
            TypeCode::NextHop          =>   3,
            TypeCode::MultiExitDisc    =>   4,
            TypeCode::LocalPref        =>   5,
            TypeCode::AtomicAggregate  =>   6,
            TypeCode::Aggregator       =>   7,
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Category {
    WellKnownMandatory,
    WellKnownDiscretionary,
    OptionalTransitive,
    OptionalNonTransitive,
}
#[derive(PartialEq, Debug, Copy, Clone)]
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

    pub fn to_u8(&self) -> u8 {
        match self.origin_type {
            OriginType::IGP => 0,
            OriginType::EGP => 1,
            OriginType::Incomplete => 2
        }
    }
}
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum OriginType {
    IGP,
    EGP,
    Incomplete
}

#[derive(PartialEq, Debug, Copy, Clone)]
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

    pub fn to_u8(&self) -> u8 {
        match self {
            AsPathSegmentType::ASSet => 1,
            AsPathSegmentType::AsSequence => 2
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum AS {
    AS2(u16),
    AS4(u32)
}

impl AS {
    // TODO handle AS2, mayble match self, and return an u32.
    // Then return an enum that shows variant used.

    pub fn to_u32(&self) -> Result<u32, ProcessError> {

        match self {
            AS::AS2(as_num) => Err(ProcessError::AS2Unhandled),
            AS::AS4(as_num) => Ok(*as_num)
        }

    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct AsPath {
    category: Category,
    as_path_segment: AsPathSegment
}

#[derive(PartialEq, Debug, Clone)]
pub struct AsPathSegment {
    pub segment_type: AsPathSegmentType, // 1 byte
    pub number_of_as: u8, // number of ASes, not number of bytes
    // TODO handle 2 vs 4 byte AS
    pub as_list: Vec<AS> // 2 or 4 bytes each
}

impl AsPath {
    pub fn to_u8_vec(&self) -> Result<Vec<u8>, ProcessError> {
        let mut bytes = Vec::new();

        // as path
        // segment type
        let asp_seg_type = self.as_path_segment.segment_type.to_u8().to_be_bytes();
        bytes.extend(asp_seg_type);

        // number of as
        let num_of_as = self.as_path_segment.number_of_as.to_be_bytes();
        bytes.extend(num_of_as);

        if self.as_path_segment.number_of_as as usize != self.as_path_segment.as_list.len() {
            return Err(ProcessError::ASNumLenMismatch)
        }

        // variable as list
        for as_num in &self.as_path_segment.as_list {
            bytes.extend(as_num.to_u32().unwrap().to_be_bytes());
        }

        Ok(bytes)
    }
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

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct NextHop {
    category: Category,
    // TODO handle other sizes
    ipv4addr: Ipv4Addr,
}

impl NextHop {
    pub fn from_vec_u8(bytes: &Vec<u8>) -> Self {
        NextHop {
            category: Category::WellKnownMandatory,
            ipv4addr: Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]),
        }
    }

}


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct MultiExitDisc {
    category: Category,
    value: u32, // 4 bytes
}

impl MultiExitDisc {
    pub fn from_vec_u8(bytes: &Vec<u8>) -> Self {
        MultiExitDisc {
            category: Category::OptionalNonTransitive,
            value: u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct LocalPref {
    category: Category,
    value: u32, // 4 bytes
}

impl LocalPref {
    pub fn from_vec_u8(bytes: &Vec<u8>) -> Self {
        LocalPref {
            category: Category::WellKnownDiscretionary,
            value: u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct AtomicAggregate {
    category: Category,
    value: u32, // 4 bytes
}


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Aggregator {
    category: Category,
    as_num: AS, // 2 bytes or 4
    ipv4addr: Ipv4Addr
}

#[derive(PartialEq, Debug, Clone)]
pub enum PAdata {
    Origin(Origin),
    AsPath(AsPath),
    NextHop(NextHop),
    MultiExitDisc(MultiExitDisc),
    LocalPref(LocalPref),
    AtomicAggregate(AtomicAggregate),
    Aggregator(Aggregator)
}


// pub fn extract_num_of_paths_from_data(bytes: &Vec<u8>) -> u8 {
//     for i in 0..bytes.len() {
//
//     }
// }

// pub fn extract_path_attributes_from_data(bytes: &Vec<u8>, total_path_att_len: u16) -> Vec<PAdata> {
//     let num_of_atts = extract_num_of_paths_from_data(bytes);
//
//     let pa_data = Vec::new();
//     for i in 0..num_of_atts {
//         //PAdata::from_vec_u8
//     }
//
//
//     pa_data
// }


impl PAdata {
    pub fn from_vec_u8(type_code: &TypeCode, bytes: &Vec<u8>) -> Self {
        match *type_code {
            TypeCode::Origin => {
                PAdata::Origin(Origin::from_u8(bytes[0]))
            },
            TypeCode::AsPath => {
                PAdata::AsPath(AsPath::from_vec_u8(bytes))
            },
            TypeCode::NextHop => {
                PAdata::NextHop(NextHop::from_vec_u8(bytes))
            },
            TypeCode::MultiExitDisc => {
                PAdata::MultiExitDisc(MultiExitDisc::from_vec_u8(bytes))
            },
            TypeCode::LocalPref => {
                PAdata::LocalPref(LocalPref::from_vec_u8(bytes))
            },
            tc => {
                panic!("Unimplemented patype code {:?}", tc);
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct PathAttribute {
    pub flags: Flags,
    pub type_code: TypeCode,
    pub len: u8,
    pub data: PAdata
}

impl PathAttribute {
    pub fn new(flags: Flags, type_code: TypeCode, len: u8, data: PAdata) -> Self {
        PathAttribute {
            flags,
            type_code,
            len,
            data
        }
    }


    // TODO add arguments here after testing is finished
    pub fn new_next_hop(nhp_ip: Ipv4Addr) -> Self {
        let mut flags = Flags::new();
        flags.transitive = Flag::Transitive(true);

        // TODO when we add in IPv6 this needs to be a match
        let len = 4;


        let next_hop = NextHop {
            category: Category::WellKnownMandatory,
            ipv4addr: nhp_ip,
        };

        let data = PAdata::NextHop(next_hop);

        PathAttribute {
            flags,
            type_code: TypeCode::NextHop,
            len,
            data,
        }
    }



    // TODO add arguments here after testing is finished
    pub fn new_as_path(as_list_vec: Vec<u32>) -> Self {
        let mut flags = Flags::new();
        flags.transitive = Flag::Transitive(true);

        // TODO replace len after testing is finished
        let len = 2 + as_list_vec.len() as u8 * 4;

        let mut as_list = Vec::new();
        for as_num in as_list_vec {
            as_list.push(AS::AS4(as_num));
        }

        let as_path_segment = AsPathSegment {
            segment_type: AsPathSegmentType::AsSequence,
            number_of_as: 1,
            as_list,
        };

        let as_path = AsPath {
            category: Category::WellKnownMandatory,
            as_path_segment
        };

        let data = PAdata::AsPath(as_path);

        PathAttribute {
            flags,
            type_code: TypeCode::AsPath,
            len,
            data,
        }
    }


    pub fn new_origin(origin_type: OriginType) -> Self {

        let mut flags = Flags::new();
        flags.transitive = Flag::Transitive(true);

        // origin len here is always 1
        let len = 1;

        let category = Category::WellKnownMandatory;

        match origin_type {
            OriginType::IGP => {
                let origin = Origin {
                    category,
                    origin_type,
                };
                let data = PAdata::Origin(origin);
                PathAttribute {
                    flags,
                    type_code: TypeCode::Origin,
                    len,
                    data,
                }
            },
            OriginType::EGP => {
                let origin = Origin {
                    category,
                    origin_type,
                };
                let data = PAdata::Origin(origin);
                PathAttribute {
                    flags,
                    type_code: TypeCode::Origin,
                    len,
                    data,
                }
            },
            OriginType::Incomplete => {
                let origin = Origin {
                    category,
                    origin_type,
                };
                let data = PAdata::Origin(origin);
                PathAttribute {
                    flags,
                    type_code: TypeCode::Origin,
                    len,
                    data,
                }
            }
        }


    }

    pub fn convert_to_bytes(&self) -> Result<Vec<u8>, ProcessError> {
        let mut pa_bytes = Vec::new();

        // flags
        let flags = self.flags.to_u8().to_be_bytes();
        pa_bytes.extend(flags);

        // type_code
        let type_code = self.type_code.to_u8().to_be_bytes();
        pa_bytes.extend(type_code);

        // len
        let len = self.len.to_be_bytes();
        pa_bytes.extend(len);

        //data variable
        match &self.data {
            PAdata::Origin(origin) => {
                pa_bytes.extend(origin.to_u8().to_be_bytes());
            },
            PAdata::AsPath(as_path) => {
                let asp = as_path.to_u8_vec()?;
                pa_bytes.extend(asp);
            },
            PAdata::NextHop(nh) => {
                pa_bytes.extend(nh.ipv4addr.to_bits().to_be_bytes());
            },
            PAdata::MultiExitDisc(med) => {
                pa_bytes.extend(med.value.to_be_bytes());
            },
            PAdata::LocalPref(lp) => {
                pa_bytes.extend(lp.value.to_be_bytes());
            },
            PAdata::AtomicAggregate(atomic_agg) => {
                pa_bytes.extend(atomic_agg.value.to_be_bytes());
            },
            PAdata::Aggregator(agg) => {
                pa_bytes.extend(agg.ipv4addr.to_bits().to_be_bytes());
            }
        }

        Ok(pa_bytes)
    }

    pub fn get_pa_data_from_pa(&self, type_code: TypeCode) -> Option<PAdata> {
        if self.type_code == type_code {
            return Some(self.data.clone())
        }
        None
    }

    pub fn get_pa_data_from_pa_vec(type_code: TypeCode, pa_vec: &Vec<PathAttribute>) -> Option<PAdata> {
        match type_code {
            TypeCode::Origin => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::Origin(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::AsPath => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::AsPath(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::NextHop => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::NextHop(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::MultiExitDisc => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::MultiExitDisc(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::LocalPref => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::LocalPref(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::AtomicAggregate => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::AtomicAggregate(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            },
            TypeCode::Aggregator => {
                for pa in pa_vec {
                    if matches!(pa.data, PAdata::Aggregator(_)) {
                        return Some(pa.data.clone())
                    }
                }
                None
            }
        }

        // if self.type_code == type_code {
        //     return Some(self.data.clone())
        // }
        // None
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct UpdateMessage {
    // min len is 23 bytes
    pub message_header: MessageHeader,
    pub withdrawn_route_len: u16, // size in bytes, not the number of objects
    pub withdrawn_routes: Option<Vec<NLRI>>,
    pub total_path_attribute_len: u16, // size in bytes, not the number of objects
    pub path_attributes: Option<Vec<PathAttribute>>,
    pub nlri: Option<Vec<NLRI>>
}

pub fn extract_update_message(tsbuf: &Vec<u8>) -> Result<UpdateMessage, MessageError> {
    // TODO I only copied this, I have not modified it yet
    println!("Extracting update message");
    let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;
    if message_len < 23 {
        return Err(MessageError::UpdateMessageLenTooLow)
    }
    //println!("message_len: {}", message_len);
    let withdrawn_route_len = extract_u16_from_bytes(tsbuf, 19, 21)?;
    //println!("withdrawn route len is : {}", withdrawn_route_len);
    let mut current_idx = 0;
    let base_idx = 21;
    let route_size = 5;
    let withdrawn_routes: Option<Vec<NLRI>> = if withdrawn_route_len >= route_size {
        //println!("withdrawn_route_len is greater than or equal to route_size");
        let mut routes: Vec<NLRI> = Vec::new();
        for x in 0.. (withdrawn_route_len / route_size) as usize {
            current_idx = base_idx + (route_size as usize * x);
            //println!("current_idx {}", current_idx);
            let prefix_len = extract_u8_from_byte(tsbuf, current_idx, current_idx + 1)?;
            //println!("prefix_len {}", prefix_len);
            current_idx += 1;

            let route_u32 = extract_u32_from_bytes(tsbuf, current_idx, current_idx + route_size as usize)?;
            //println!("route_u32 {}", route_u32);
            match NLRI::new(Ipv4Addr::from_bits(route_u32), prefix_len) {
                Ok(nl) => { routes.push(nl); },
                Err(e) => {
                    println!("Error: {:#?}", e);
                }
            }
            // regardless we need to inc the idx
            current_idx += 4;
            //println!("current_idx {}", current_idx);
        }
        Some(routes)
    } else {
        None
    };

    let mut current_idx = base_idx + withdrawn_route_len as usize;
    //println!("current_idx {}", current_idx);

    let total_path_attribute_len = extract_u16_from_bytes(tsbuf, current_idx, current_idx + 2)?;
    //println!("total_path_attribute_len {}", total_path_attribute_len);
    current_idx += 2;
    //println!("current_idx {}", current_idx);

        // origin, aspath, next hop are the mandatory atts (24 total).

    // losing the order of the PAs shouldn't matter because we process each one into an object right after finding it, so an option of vec is fine
    let path_attributes: Option<Vec<PathAttribute>> = if total_path_attribute_len > 0 {
        let mut pa_collection = Vec::new();
        // The PAs are variable length here so we need to parse until we've read the whole msg len
        let mut pa_idx: usize = 0;
        while pa_idx < total_path_attribute_len as usize {
            //println!("current_idx is less than message len, extracting path attributes");

            let flags = {
                // TODO handle these errors so we continue instead of returning from the func
                let val = extract_u8_from_byte(tsbuf, current_idx, current_idx + 1)?;
                Flags::from_u8(val)
            };
            //println!("flags {:#?}", flags);

            current_idx += 1;
            pa_idx += 1;
            //println!("current_idx {}", current_idx);

            let type_code = {
                let val = extract_u8_from_byte(tsbuf, current_idx, current_idx + 1)?;
                TypeCode::from_u8(val)
            };
            //println!("type_code {:#?}", type_code);

            current_idx += 1;
            pa_idx += 1;
            //println!("current_idx {}", current_idx);

            let len = extract_u8_from_byte(tsbuf, current_idx, current_idx + 1)?;
            current_idx += 1;
            pa_idx += 1;
            //println!("current_idx {}", current_idx);

            let data_bytes = {
                let mut bytes: Vec<u8> = Vec::new();
                for x in 0..len as usize {
                    match tsbuf.get(current_idx + x) {
                        Some(byte) => { bytes.push(*byte); },
                        None => { println!("{:#?}", MessageError::InvalidBufferIndex) } // should not return error, just pass
                    }
                }
                bytes
            };
            //println!("data_bytes {:#?}", data_bytes);

            current_idx += len as usize;
            pa_idx += len as usize;
            //println!("current_idx {}", current_idx);

            // parse the bytes we just read for the PA
            //let data = extract_path_attributes_from_data(&data_bytes, total_path_attribute_len);
            if !data_bytes.is_empty() {
                // extract the PAdata object from the vec of bytes and create a new PathAtrribute object to be returned
                let pa_data = PAdata::from_vec_u8(&type_code, &data_bytes);
                //println!("pa_data is  {:#?}", pa_data);

                pa_collection.push(PathAttribute::new(flags, type_code, len, pa_data));
            }
        }
        //println!("Option<Vec<PathAttribute>> has Some");
        Some(pa_collection)
    } else {
        //println!("Option<Vec<PathAttribute>> has None");
        None
    };

    if current_idx == message_len as usize {
        //println!("No NLRI in this message because current_idx == message_len");
        return Err(MessageError::MissingNLRI)
    }

    let nlri = extract_nlri_from_update_message(tsbuf, message_len as usize, &mut current_idx);
    // TODO read and process the optional params
    let update_message = UpdateMessage::new(message_len, withdrawn_route_len, withdrawn_routes, total_path_attribute_len, path_attributes, nlri)?;

    Ok(update_message)

}

pub fn extract_nlri_from_update_message(tsbuf: &Vec<u8>, message_len: usize, mut current_idx: &mut usize) -> Option<Vec<NLRI>> {
    let route_size: usize = 5;
    let mut nlri = Vec::new();
    let nlri_count: usize = (message_len - *current_idx) / route_size;
    //println!("message_len is  {:#?}", message_len);
    //println!("current_idx is  {:#?}", current_idx);
    //println!("nlri_count is  {:#?}", nlri_count);

    for i in 0..nlri_count {
        let len: Option<u8> = {
            match tsbuf.get(*current_idx) {
                Some(l) => {
                    *current_idx += 1;
                    //println!("Some(l) is  {:#?}", l);
                    Some(l.clone())
                },
                None => None
            }
        };

       let prefix = match tsbuf.get(*current_idx..=*current_idx + 3) {
            Some(p) => {
                *current_idx += 4;
                //println!("Some(p) is  {:#?}", p);
                Some(p)
            },
            None => None
        };
        if len.is_some() && prefix.is_some() {
            //println!("creating nlri struct");
            let rt = NLRI {
                len: len.unwrap(),
                //prefix: u32::from_be_bytes(prefix.unwrap().try_into().unwrap())
                prefix: Ipv4Addr::from_bits(u32::from_be_bytes(prefix.unwrap().try_into().unwrap()))
            };
            //println!("rt is  {:#?}", rt);
            nlri.push(rt);
        }
    }
    if nlri.is_empty() {
        //println!("returning None for NLRI vec");
        None
    } else {
        //println!("returning NLRI vec");
        Some(nlri)
    }
}

impl UpdateMessage {
    pub fn new(message_len: u16, withdrawn_route_len: u16, withdrawn_routes: Option<Vec<NLRI>>, total_path_attribute_len: u16, path_attributes: Option<Vec<PathAttribute>>, nlri: Option<Vec<NLRI>> ) -> Result<Self, MessageError> {
        let message_header = MessageHeader::new(MessageType::Update, Some(message_len))?;

        Ok(UpdateMessage {
            message_header,
            withdrawn_route_len,
            withdrawn_routes,
            total_path_attribute_len,
            path_attributes,
            nlri
        })

    }

    pub fn convert_to_bytes(&self) -> Vec<u8> {
        let mut message: Vec<u8> = vec![0xFF; 16];

        // marker
        let mut len: u16 = message.len() as u16;
        len += 2;

        let message_type = MessageType::Update;
        let msg_type: u8 = message_type.to_u8();
        len += 1;


        // withdrawn routes len.
        let wr_len_bytes = self.withdrawn_route_len.to_be_bytes();
        len += 2;

        // 21

        //variable withdrawn routes
        let mut wr_vec_len: u16 = 0;
        let mut withdrawn_routes_bytes: Vec<u8> = Vec::new();
        if self.withdrawn_route_len > 0 && self.withdrawn_routes.is_some() {
            // can't trust anything since these updates may be generated by someone fuzzing
            let wr_vec = self.withdrawn_routes.as_ref().unwrap();
            wr_vec_len = wr_vec.len() as u16;
            // TODO remove unwrap and handle
            for wr in wr_vec {
                withdrawn_routes_bytes.extend(wr.convert_to_bytes());
            }
        }
        len += wr_vec_len;



        // total path att. len
        let path_att_len_bytes = self.total_path_attribute_len.to_be_bytes();
        len += 2;

        // 23

        //variable path atts
        let mut path_att_vec_len: u16 = 0;
        let mut path_att_bytes: Vec<u8> = Vec::new();
        if self.total_path_attribute_len > 0 && self.path_attributes.is_some() {
            let path_att_vec = self.path_attributes.as_ref().unwrap();
            //path_att_vec_len = path_att_vec.len() as u16;
                for pa in path_att_vec {
                    match pa.convert_to_bytes() {
                        Ok(pa_bytes) => {
                            path_att_bytes.extend(pa_bytes);
                        },
                        Err(e) => {
                            println!("Error converting PA to bytes: {:#?}", e);
                        }
                    }
                }
        }

        len += path_att_bytes.len() as u16;


        let mut nlri_bytes: Vec<u8> = Vec::new();
        // not a field but will be multiplied by 5 and added to len of the update message
        let mut nlri_len: u16 = 0;
        // actual nlri field(s)

        // I guess I could also just as_ref() and .unwrap() here
        if let Some(nlri) = &self.nlri {
            nlri_len = nlri.len() as u16;
            for n in nlri {
                nlri_bytes.extend(n.convert_to_bytes());
            }
        }
        len += nlri_len * 5;

        // adding len to the vec must come second to last because we need the total len of the payload
        let len_bytes = len.to_be_bytes();
        message.extend(len_bytes);

        message.push(msg_type);

        message.extend(wr_len_bytes);

        message.extend(withdrawn_routes_bytes);

        message.extend(path_att_len_bytes);

        message.extend(path_att_bytes);

        message.extend(nlri_bytes);

        message
    }
}
