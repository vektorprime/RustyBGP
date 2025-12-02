use crate::errors::BGPError::Message;

#[derive(PartialEq, Debug)]
pub enum NeighborError {
    KeepaliveGreaterThanHoldTime,
    KeepaliveEqualToHoldTime,
    PeerIPNotRecognized,
    NeighborIsIPV6,
    ConfiguredNeighborsEmpty,
    PeerIPNotEstablished,
    Message(MessageError)
}

#[derive(PartialEq, Debug)]
pub enum MessageError {
    NoMarkerFound,
    NoLenFound,
    BufferEmpty,
    UnknownMessageType,
    UnableToWriteToTCPStream,
    RouteRefreshMissingAFI,
    BadInt8Read,
    BadInt16Read,
    BadInt32Read,
    InvalidBufferIndex,
    BadBGPVersion,
    PeerIPNotEstablished,
    MissingNLRI
}

#[derive(PartialEq, Debug)]
pub enum BGPError {
    Neighbor(NeighborError),
    Message(MessageError),
}
impl From<NeighborError> for BGPError {
    fn from(e: NeighborError) -> BGPError {
        BGPError::Neighbor(e)
    }
}

impl From<MessageError> for BGPError {
    fn from(e: MessageError) -> BGPError {
        BGPError::Message(e)
    }
}


impl From<MessageError> for NeighborError {
    fn from(e: MessageError) -> NeighborError {NeighborError::Message(e)}
}
