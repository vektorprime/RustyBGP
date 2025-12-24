use std::net::{IpAddr, Ipv4Addr};
use tokio::net::{TcpStream, TcpListener};
//use std::io::{Read, Write};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use std::path::Path;
use crate::errors::*;
use crate::finite_state_machine::State;
use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
use crate::neighbors::{Neighbor, PeerType};
use crate::routes::NLRI;
use crate::utils::*;


pub fn extract_open_message(tsbuf: &Vec<u8>) -> Result<OpenMessage, MessageError> {
    // doesn't make sense to save the len in the messsage header here because it should be calculated in the Message::new()
    // let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;
    //
    // let mut message_header = MessageHeader::new(MessageType::Open, message_len);
    // message_header.length = message_len;

    let version = match tsbuf.get(19) {
        Some(byte) => { BGPVersion::from_u8(byte.clone())},
        None => { return Err(MessageError::BadBGPVersion) }
    };

    if version != BGPVersion::V4 {
        return Err(MessageError::BadBGPVersion)
    }

    let as_number = extract_u16_from_bytes(tsbuf, 20, 22)?;

    let hold_time = extract_u16_from_bytes(tsbuf, 22, 24)?;

    let identifier = Ipv4Addr::from_bits(extract_u32_from_bytes(tsbuf, 30, 34)?);

    // TODO read and process the optional params
    let open_message = OpenMessage::new(BGPVersion::V4, as_number, hold_time, identifier, 0, None)?;

    Ok(open_message)

}


// pub fn add_neighbor_from_message(bgp_proc: &mut BGPProcess, open_message: &mut OpenMessage, peer_ip: Ipv4Addr, hello_time: u16, my_hold_time: u16) -> Result<(usize), BGPError> {
//     // for an in &bgp_proc.active_neighbors {
//     //     if peer_ip.to_string() == an.ip {
//     //         return Err(NeighborError::NeighborAlreadyEstablished.into())
//     //     }
//     // }
//
//     let hold_time = if my_hold_time <= open_message.hold_time {
//         my_hold_time
//     } else {
//         open_message.hold_time
//     };
//
//     let peer_type = if bgp_proc.my_as == open_message.as_number {
//         PeerType::Internal
//     }  else {
//         PeerType::External
//     };
//
//     let neighbor = Neighbor::new(peer_ip, AS::AS2(open_message.as_number), hello_time, hold_time, peer_type)?;
//     let index = bgp_proc.neighbors.len();
//     //bgp_proc.active_neighbors.push(neighbor);
//     bgp_proc.neighbors.insert(peer_ip, neighbor);
//     Ok(index)
// }


pub fn get_neighbor_ipv4_address_from_stream(tcp_stream: &TcpStream) -> Result<Ipv4Addr, NeighborError> {
    match tcp_stream.peer_addr().unwrap().ip() {
        IpAddr::V4(ip) => Ok(ip),
        _ => { return Err(NeighborError::NeighborIsIPV6.into())}
    }
}


pub async fn test_net_advertisements(bgp_proc: &mut BGPProcess, tcp_stream: &mut TcpStream) -> Result<(), MessageError> {
    let mut path_attributes: Vec<PathAttribute> = Vec::new();

    let origin_pa = PathAttribute::new_origin(OriginType::IGP);
    path_attributes.push(origin_pa);

    let as_list = vec![2];
    let as_path_pa = PathAttribute::new_as_path(as_list);
    path_attributes.push(as_path_pa);

    let next_hop_pa = PathAttribute::new_next_hop(Ipv4Addr::new(10, 0, 0, 3));
    path_attributes.push(next_hop_pa);

    let mut nlri: Vec<NLRI> = Vec::new();
    for net in &bgp_proc.configured_networks {
        nlri.push(net.nlri.clone());
    }
    //let nlri = vec![NLRI::new(Ipv4Addr::new(1,1,1,1), 32).unwrap()];

    //let msg_len = 53;
    let message_header = MessageHeader::new(MessageType::Update, None)?;
    let update_message = UpdateMessage {
        message_header,
        withdrawn_route_len: 0,
        withdrawn_routes: None,
        total_path_attribute_len: 20,
        path_attributes: Some(path_attributes),
        nlri: Some(nlri),
    };
    send_update(tcp_stream, update_message).await?;
    
    Ok(())
}


pub async fn send_open(stream: &mut TcpStream, message: OpenMessage) -> Result<(), MessageError> {
    println!("Preparing to send Open");
    let message_bytes = message.convert_to_bytes();
    let res  =stream.write_all(&message_bytes[..]).await;
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

pub async fn send_update(stream: &mut TcpStream, message: UpdateMessage) -> Result<(), MessageError> {
    println!("Preparing to send Update");
    let message_bytes = message.convert_to_bytes();
    let res  =stream.write_all(&message_bytes[..]).await;
    match res {
        Ok(_) => {
            println!("Sent Update");
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
    pub fn new(version: BGPVersion, as_number: u16, hold_time: u16, identifier: Ipv4Addr, optional_parameters_length: u8, optional_parameters: Option<Vec<OptionalParameter>> ) -> Result<Self, MessageError> {
        // 28 bytes base without params
        let message_header_len_field = 28 + optional_parameters_length as u16;
        let message_header = MessageHeader::new(MessageType::Open, Some(message_header_len_field))?;
        Ok(OpenMessage {
            message_header,
            version,
            as_number,
            hold_time,
            identifier,
            optional_parameters_length,
            optional_parameters
        })
    }
    pub fn convert_to_bytes(&self) -> Vec<u8> {
        //let message_header_len_field = 28 + optional_parameters_length as u16;
        //let message_header = MessageHeader::new(MessageType::Open, Some(message_header_len_field));
       // let message_header_marker: [u8; 16] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        //let message_header = build_message_header(MessageType::Open);

        //let mut message: Vec<u8> = Vec::new();

        // for b in message_header_marker {
        //     message.push(b);
        // }

        let mut message: Vec<u8> = vec![0xFF; 16];
        // marker
        let mut len: u16 = message.len() as u16;
        len += 2;

        let message_type = MessageType::Open;

        let msg_type: u8 = message_type.to_u8();
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
