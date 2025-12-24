use tokio::io;

// pub fn convert_u16_to_u8(val: u16) -> [u8; 2] {
//
// }

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpStream;
use crate::errors::{BGPError, MessageError, NeighborError};

pub fn extract_u32_from_bytes(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u32, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u32::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt32Read)?)) },
        None => { Err(MessageError::BadInt32Read) }
    }
}

pub fn extract_u16_from_bytes(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u16, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u16::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt16Read)?)) },
        None => { Err(MessageError::BadInt16Read) }
    }
}

pub fn extract_u8_from_byte(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u8, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u8::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt8Read)?)) },
        None => { Err(MessageError::BadInt8Read) }
    }
}


pub fn get_neighbor_ipv4_address(peer_ip: IpAddr) -> Result<Ipv4Addr, NeighborError> {
    if let IpAddr::V4(ip) = peer_ip  {
        Ok(ip)
    }
    else {
         Err(NeighborError::NeighborIsIPV6.into())
    }
}


pub fn get_neighbor_ipv4_address_from_socket(peer_ip: io::Result<SocketAddr>) -> Result<Ipv4Addr, NeighborError> {
    match peer_ip {
        Ok(ip) => {
            match ip.ip() {
                IpAddr::V4(ip4) => Ok(ip4),
                IpAddr::V6(ip6) => Err(NeighborError::NeighborIsIPV6)
            }
        },
        Err(_) => {
            Err(NeighborError::PeerIPNotRecognized)
        }
    }

}