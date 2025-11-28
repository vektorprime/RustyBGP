
#[derive(PartialEq, Debug)]
pub enum NeighborError {
    KeepaliveGreaterThanHoldTime,
    KeepaliveEqualToHoldTime,
}

#[derive(PartialEq, Debug)]
pub enum MessageError {
    NoMarkerFound,
    NoLenFound,
    BufferEmpty,
    UnknownMessageType,
    UnableToWriteToTCPStream,
    RouteRefreshMissingAFI,
    BadIntRead,
    InvalidBufferIndex
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