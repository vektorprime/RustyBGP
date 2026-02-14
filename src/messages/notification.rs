use std::net::Ipv4Addr;
use tokio::net::TcpStream;
use std::convert::TryFrom;

use crate::errors::MessageError;
use crate::messages::{BGPVersion, MessageType};
use crate::messages::header::MessageHeader;
use crate::messages::keepalive::send_keepalive;
use crate::messages::optional_parameters::Capability;
use crate::messages::update::{extract_nlri_from_update_message, Flags, PAdata, PathAttribute, TypeCode, UpdateMessage};
use crate::process::BGPProcess;
use crate::routes::NLRI;
use crate::utils::{extract_u16_from_bytes, extract_u32_from_bytes, extract_u8_from_bytes};

#[derive(PartialEq, Debug, Clone)]
pub enum NotifErrorCode {
    MessageHeader,
    OpenMessage,
    UpdateMessage,
    HoldTimerExpired,
    FSM,
    Cease,
}


impl TryFrom<u8> for NotifErrorCode {
    type Error = MessageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(NotifErrorCode::MessageHeader),
            2 => Ok(NotifErrorCode::OpenMessage),
            3 => Ok(NotifErrorCode::UpdateMessage),
            4 => Ok(NotifErrorCode::HoldTimerExpired),
            5 => Ok(NotifErrorCode::FSM),
            6 => Ok(NotifErrorCode::Cease),
            _ => Err(MessageError::BadNotifErrorCode)

        }
    }
}

impl TryFrom<&[u8]> for NotifErrorCode {
    type Error = MessageError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value[0] {
            1 => Ok(NotifErrorCode::MessageHeader),
            2 => Ok(NotifErrorCode::OpenMessage),
            3 => Ok(NotifErrorCode::UpdateMessage),
            4 => Ok(NotifErrorCode::HoldTimerExpired),
            5 => Ok(NotifErrorCode::FSM),
            6 => Ok(NotifErrorCode::Cease),
            _ => Err(MessageError::BadNotifErrorCode)

        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum NotifErrorSubCode {
    MsgHdr(NotifErrorMsgHdrSubCode),
    Open(NotifErrorOpenSubCode),
    Update(NotifErrorUpdateSubCode),
    HoldTimerExpired,
    FSM,
    Cease
}

#[derive(PartialEq, Debug, Clone)]
pub enum NotifErrorMsgHdrSubCode {
    // header
    ConnectionNotSynchronized,
    BadMessageLength,
    BadMessageType,
}

impl TryFrom<u8> for NotifErrorMsgHdrSubCode {
    type Error = MessageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // header
            1 => Ok(NotifErrorMsgHdrSubCode::ConnectionNotSynchronized),
            2 => Ok(NotifErrorMsgHdrSubCode::BadMessageLength),
            3 => Ok(NotifErrorMsgHdrSubCode::BadMessageType),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}

impl TryFrom<&[u8]> for NotifErrorMsgHdrSubCode {
    type Error = MessageError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value[0] {
            // header
            1 => Ok(NotifErrorMsgHdrSubCode::ConnectionNotSynchronized),
            2 => Ok(NotifErrorMsgHdrSubCode::BadMessageLength),
            3 => Ok(NotifErrorMsgHdrSubCode::BadMessageType),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}


#[derive(PartialEq, Debug, Clone)]
pub enum NotifErrorOpenSubCode {
    // open
    UnsupportedVersionNumber,
    BadPeerAS,
    BadBGPIdentifier,
    UnsupportedOptionalParameter,
    DeprecatedSubCode,
    UnacceptableHoldTime,
}

impl TryFrom<u8> for NotifErrorOpenSubCode {
    type Error = MessageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(NotifErrorOpenSubCode::UnsupportedVersionNumber),
            2 => Ok(NotifErrorOpenSubCode::BadPeerAS),
            3 => Ok(NotifErrorOpenSubCode::BadBGPIdentifier),
            4 => Ok(NotifErrorOpenSubCode::UnsupportedOptionalParameter),
            5 => Ok(NotifErrorOpenSubCode::DeprecatedSubCode),
            6 => Ok(NotifErrorOpenSubCode::UnacceptableHoldTime),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}

impl TryFrom<&[u8]> for NotifErrorOpenSubCode {
    type Error = MessageError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value[0] {
            1 => Ok(NotifErrorOpenSubCode::UnsupportedVersionNumber),
            2 => Ok(NotifErrorOpenSubCode::BadPeerAS),
            3 => Ok(NotifErrorOpenSubCode::BadBGPIdentifier),
            4 => Ok(NotifErrorOpenSubCode::UnsupportedOptionalParameter),
            5 => Ok(NotifErrorOpenSubCode::DeprecatedSubCode),
            6 => Ok(NotifErrorOpenSubCode::UnacceptableHoldTime),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum NotifErrorUpdateSubCode {
    // update
    MalformedAttributeList,
    UnrecognizedWellKnownAttribute,
    MissingWellKnownAttribute,
    AttributeFlagsError,
    AttributeLengthError,
    InvalidOriginAttribute,
    DeprecatedSubCode,
    InvalidNextHopAttribute,
    OptionalAttributeError,
    InvalidNetworkField,
    MalformedASPath,
}

impl TryFrom<u8> for NotifErrorUpdateSubCode {
    type Error = MessageError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(NotifErrorUpdateSubCode::MalformedAttributeList),
            2 => Ok(NotifErrorUpdateSubCode::UnrecognizedWellKnownAttribute),
            3 => Ok(NotifErrorUpdateSubCode::MissingWellKnownAttribute),
            4 => Ok(NotifErrorUpdateSubCode::AttributeFlagsError),
            5 => Ok(NotifErrorUpdateSubCode::AttributeLengthError),
            6 => Ok(NotifErrorUpdateSubCode::InvalidOriginAttribute),
            7 => Ok(NotifErrorUpdateSubCode::DeprecatedSubCode),
            8 => Ok(NotifErrorUpdateSubCode::InvalidNextHopAttribute),
            9 => Ok(NotifErrorUpdateSubCode::OptionalAttributeError),
            10 => Ok(NotifErrorUpdateSubCode::InvalidNetworkField),
            11 => Ok(NotifErrorUpdateSubCode::MalformedASPath),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}

impl TryFrom<&[u8]> for NotifErrorUpdateSubCode {
    type Error = MessageError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value[0] {
            1 => Ok(NotifErrorUpdateSubCode::MalformedAttributeList),
            2 => Ok(NotifErrorUpdateSubCode::UnrecognizedWellKnownAttribute),
            3 => Ok(NotifErrorUpdateSubCode::MissingWellKnownAttribute),
            4 => Ok(NotifErrorUpdateSubCode::AttributeFlagsError),
            5 => Ok(NotifErrorUpdateSubCode::AttributeLengthError),
            6 => Ok(NotifErrorUpdateSubCode::InvalidOriginAttribute),
            7 => Ok(NotifErrorUpdateSubCode::DeprecatedSubCode),
            8 => Ok(NotifErrorUpdateSubCode::InvalidNextHopAttribute),
            9 => Ok(NotifErrorUpdateSubCode::OptionalAttributeError),
            10 => Ok(NotifErrorUpdateSubCode::InvalidNetworkField),
            11 => Ok(NotifErrorUpdateSubCode::MalformedASPath),
            _ => Err(MessageError::BadNotifErrorSubCode)
        }
    }
}
#[derive(PartialEq, Debug, Clone)]
pub struct NotificationMessage {
    // min length 21 bytes without variable-length data
    // msg len is 21 + data length
    pub message_header: MessageHeader,
    pub error: NotifErrorCode,
    pub error_subcode: NotifErrorSubCode,
    pub data: Option<Vec<u8>>
}

impl NotificationMessage {
    pub fn new(error: NotifErrorCode, error_subcode: NotifErrorSubCode, data: Option<Vec<u8>>) -> Result<Self, MessageError> {
        let msg_hdr_len: Option<u16> = if let Some(d) = data.as_ref() {
            if d.len() + 21 > 65535 {
                return Err(MessageError::MessageLenTooBig)
            } else {
                Some(21 + d.len() as u16)
            }
        } else {
            None
        };
        let message_header = MessageHeader::new(MessageType::Notification, msg_hdr_len)?;
        Ok(NotificationMessage {
            message_header,
            error,
            error_subcode,
            data
        })
    }
}


pub fn extract_notification_message(tsbuf: &Vec<u8>) -> Result<NotificationMessage, MessageError> {
    println!("Extracting notification message");
    let message_len = extract_u16_from_bytes(tsbuf, 16, 18)?;
    if message_len < 21 {
        return Err(MessageError::MessageLenTooLow)
    }

    let mut current_idx: usize = 19;

    let error_code = match tsbuf.get(current_idx..=current_idx + 1) {
        Some(ec) => {
            current_idx += 1;
            //println!("Some(ec) is  {:#?}", ec);
            NotifErrorCode::try_from(ec)?
        },
        None => { return Err(MessageError::BadNotifErrorCode) }
    };

    let error_subcode = match tsbuf.get(current_idx..=current_idx + 1) {
        Some(esc) => {
            current_idx += 1;
            match error_code {
                NotifErrorCode::MessageHeader => {
                    let sub_code = NotifErrorMsgHdrSubCode::try_from(esc)?;
                    NotifErrorSubCode::MsgHdr(sub_code)
                },
                NotifErrorCode::OpenMessage => {
                    let sub_code = NotifErrorOpenSubCode::try_from(esc)?;
                    NotifErrorSubCode::Open(sub_code)
                },
                NotifErrorCode::UpdateMessage => {
                    let sub_code = NotifErrorUpdateSubCode::try_from(esc)?;
                    NotifErrorSubCode::Update(sub_code)
                },
                NotifErrorCode::HoldTimerExpired => {
                    NotifErrorSubCode::HoldTimerExpired
                },
                NotifErrorCode::FSM => {
                    NotifErrorSubCode::FSM
                },
                NotifErrorCode::Cease => {
                    NotifErrorSubCode::Cease
                },
            }

        },
        None => { return Err(MessageError::BadNotifErrorSubCode) }
    };

    let data = if current_idx == message_len as usize {
        None
    } else {
        // todo handle variable data in notif
        let mut dt = Vec::new();
        Some(dt)
    };

    match NotificationMessage::new(error_code, error_subcode, data) {
        Ok(msg) => {
            Ok(msg)
        },
        Err(err) => Err(err)
    }





}

pub async fn handle_notification_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), MessageError> {
    //send_keepalive(tcp_stream).await?;
    // TODO
    Ok(())
}