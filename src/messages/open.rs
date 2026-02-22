use std::net::{IpAddr, Ipv4Addr};
use tokio::net::{TcpStream, TcpListener};
//use std::io::{Read, Write};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use std::path::Path;
use tokio::net::tcp::OwnedWriteHalf;
use crate::config::MultiProtocolExtensionsConfig;
use crate::errors::*;
use crate::errors::MessageError::BadMultiProtocolExtValue;
use crate::finite_state_machine::State;
use crate::messages::header::*;
use crate::messages::keepalive::*;
use crate::messages::*;
use crate::messages::optional_parameters::{Capability, MPExtVal, OptionalParameters};
use crate::neighbors::{Neighbor, PeerType};
use crate::routes::NLRI;
use crate::utils::*;


pub fn extract_open_message(tsbuf: &Vec<u8>) -> Result<OpenMessage, MessageError> {
    // doesn't make sense to save the len in the messsage header here because it should be calculated in the Message::new()
    let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;

    if message_len as usize != tsbuf.len() {
        return Err(MessageError::UpdateMessageLenAndIdxMismatch);
    }
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

    let identifier = Ipv4Addr::from_bits(extract_u32_from_bytes(tsbuf, 24, 28)?);

    let opt_param_len = extract_u8_from_bytes(tsbuf, 28, 29)?;
    if opt_param_len as usize != tsbuf.len() - 29 {
        return Err(MessageError::UpdateMessageLenAndIdxMismatch);
    }
    let mut opt_curr_idx = 29;
    //let opt_start_idx = 29;
    // 4 bytes is the min for the optional param
    let optional_parameters = if opt_param_len >= 4 {
        let mut capabilities: Vec<Capability> = Vec::new();
        while opt_curr_idx < tsbuf.len() {
            // param type 2 is capability
            let param_type = extract_u8_from_bytes(tsbuf, opt_curr_idx, opt_curr_idx + 1)?;
            if param_type != 2 {
                return Err(MessageError::UnableToExtractOptionalParameters)
            }
            opt_curr_idx += 1;

            let param_len = extract_u8_from_bytes(tsbuf, opt_curr_idx, opt_curr_idx + 1)?;
            opt_curr_idx += 1;

            let mut cap_curr_idx = opt_curr_idx;
            // TODO Add the rest of the capabilities like Route Refresh, for now I just want to see the AS4 capability and multiprotocol
            let cap_type = extract_u8_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 1)?;
            cap_curr_idx += 1;

            // 1 is multi protocol extensions
            if cap_type == 1 {

                // no good use for len here yet unless there's a variable length payload in afi or safi, I'll leave this in for now
                let mp_ex_cap_len = extract_u8_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 1)?;
                cap_curr_idx += 1;

                // 1 is ipv4, 2 is ipv6 https://www.iana.org/assignments/address-family-numbers/address-family-numbers.xhtml
                let mp_ex_afi_type = extract_u16_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 2)?;
                // skip 3 bytes because 2 bytes for the u16 and the third byte is a reserved field
                cap_curr_idx += 3;

                // 1 is unicast 2 is multicast 128 is vpn https://www.iana.org/assignments/safi-namespace/safi-namespace.xhtml#safi-namespace-2
                let mp_ex_safi_type = extract_u8_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 1)?;
                cap_curr_idx += 1;


                let mp_ext_val = MPExtVal::new(mp_ex_afi_type, mp_ex_safi_type)?;
                capabilities.push(Capability::MultiprotocolExtensions(mp_ext_val));
            }

            // 65 is AS4
            if cap_type == 65 {
                let as_ex_cap_len = extract_u8_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 1)?;
                cap_curr_idx += 1;

                let as_num = extract_u32_from_bytes(tsbuf, cap_curr_idx, cap_curr_idx + 4)?;
                cap_curr_idx += 4;

                capabilities.push(Capability::Extended4ByteASN(as_num));
            }

            opt_curr_idx += param_len as usize;
        }
        Some(OptionalParameters { capabilities })
    } else { None };

    // TODO read and process the optional params
    let open_message = OpenMessage::new(BGPVersion::V4, as_number, hold_time, identifier, opt_param_len, optional_parameters)?;

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


// pub async fn test_net_advertisements(bgp_proc: &mut BGPProcess, tcp_stream: &mut TcpStream) -> Result<(), MessageError> {
//     let mut path_attributes: Vec<PathAttribute> = Vec::new();
//
//     let origin_pa = PathAttribute::new_origin(OriginType::IGP);
//     path_attributes.push(origin_pa);
//
//     let as_list = vec![2];
//     let as_path_pa = PathAttribute::new_as_path(as_list);
//     path_attributes.push(as_path_pa);
//
//     let next_hop_pa = PathAttribute::new_next_hop(Ipv4Addr::new(10, 0, 0, 3));
//     path_attributes.push(next_hop_pa);
//
//     let mut nlri: Vec<NLRI> = Vec::new();
//     for net in &bgp_proc.configured_networks {
//         nlri.push(net.nlri.clone());
//     }
//     //let nlri = vec![NLRI::new(Ipv4Addr::new(1,1,1,1), 32).unwrap()];
//
//     //let msg_len = 53;
//     let message_header = MessageHeader::new(MessageType::Update, None)?;
//     let update_message = UpdateMessage {
//         message_header,
//         withdrawn_route_len: 0,
//         withdrawn_routes: None,
//         total_path_attribute_len: 20,
//         path_attributes: Some(path_attributes),
//         nlri: Some(nlri),
//     };
//     send_update(tcp_stream, update_message).await?;
//
//     Ok(())
// }


pub async fn send_open(stream: &mut OwnedWriteHalf, message: OpenMessage) -> Result<(), MessageError> {
    println!("Preparing to send Open");
    match message.convert_to_bytes() {
        Ok(message_bytes) => {
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
        },
        Err(e) => Err(e)
    }


}

pub async fn send_update(stream: &mut OwnedWriteHalf, message: UpdateMessage, capabilities: &Option<Vec<Capability>>) -> Result<(), MessageError> {
    println!("Preparing to send Update");
    let message_bytes = message.convert_to_bytes(capabilities);
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

#[derive(PartialEq, Debug, Clone)]
pub struct OpenMessage {
    // min length 29 bytes
    pub message_header: MessageHeader,
    pub version: BGPVersion, // only V4 is in use
    pub as_number: u16,
    pub hold_time: u16, // either 0 or at least 3 sec, lowest between the two peers
    pub identifier: Ipv4Addr,
    pub optional_parameters_length: u8,
    pub optional_parameters: Option<OptionalParameters>,
}


impl OpenMessage {
    pub fn new(version: BGPVersion, as_number: u16, hold_time: u16, identifier: Ipv4Addr, optional_parameters_length: u8, optional_parameters: Option<OptionalParameters>) -> Result<Self, MessageError> {
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

    pub fn convert_to_bytes(&self) -> Result<Vec<u8>, MessageError> {
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

        let mut capabilities = Vec::new();
        if let Some(opt_params) = &self.optional_parameters {
            for capability in &opt_params.capabilities {
                let cap_bytes = capability.convert_to_bytes()?;
                capabilities.extend(cap_bytes);
            }
        }

        len += 1; // for the len field itself
        len += capabilities.len() as u16; // for the params


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
        //message.push(opt_params_len);
        let opt_param_len = (capabilities.len() as u8).to_be_bytes();
        message.extend(opt_param_len);
        message.extend(capabilities);
        // let opt_params: [u8; 28] = [0x02, 0x06, 0x01, 0x04, 0x00, 0x01, 0x00, 0x01, 0x02, 0x02, 0x80, 0x00, 0x02, 0x02, 0x02, 0x00, 0x02, 0x02, 0x46, 0x00, 0x02, 0x06, 0x41, 0x04, 0x00, 0x00, 0x00, 0x01];
        // message.extend_from_slice(&opt_params);
        // let opt_params: [u8; 4] = [0x02, 0x06, 0x41, 0x04];
        // message.extend_from_slice(&opt_params);


        Ok(message)
    }
}
