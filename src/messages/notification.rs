use std::net::Ipv4Addr;
use tokio::net::TcpStream;
use crate::errors::MessageError;
use crate::messages::{BGPVersion, OptionalParameter};
use crate::messages::header::MessageHeader;
use crate::messages::keepalive::send_keepalive;
use crate::process::BGPProcess;

#[derive(PartialEq, Debug, Clone)]
pub struct NotificationMessage {
    // min length 29 bytes
    pub message_header: MessageHeader,
    pub version: BGPVersion, // only V4 is in use
    pub as_number: u16,
    pub hold_time: u16, // either 0 or at least 3 sec, lowest between the two peers
    pub identifier: Ipv4Addr,
    pub optional_parameters_length: u8,
    pub optional_parameters: Option<Vec<OptionalParameter>>,
}



pub async fn handle_notification_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), MessageError> {
    //send_keepalive(tcp_stream).await?;
    // TODO
    Ok(())
}