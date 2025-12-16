use crate::errors::BGPError::Message;

#[derive(PartialEq, Debug)]
pub enum NeighborError {
    KeepaliveGreaterThanHoldTime,
    KeepaliveEqualToHoldTime,
    PeerIPNotRecognized,
    NeighborIsIPV6,
    ConfiguredNeighborsEmpty,
    PeerIPNotEstablished,
    NeighborAlreadyEstablished,
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
    MissingNLRI,
    MissingPathAttributes,
    MessageHeaderBadLen
}

#[derive(PartialEq, Debug)]
pub enum ProcessError {
    BadNLRILen,
    AS2Unhandled,
    ASNumLenMismatch,
}

#[derive(PartialEq, Debug)]
pub enum BGPError {
    Neighbor(NeighborError),
    Message(MessageError),
    Process(ProcessError),
}

impl From<ProcessError> for BGPError {
    fn from(e: ProcessError)  -> BGPError { BGPError::Process(e) }
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
