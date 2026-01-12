use crate::errors::BGPError::Message;

#[derive(PartialEq, Debug)]
pub enum NeighborError {
    KeepaliveGreaterThanHoldTime,
    KeepaliveEqualToHoldTime,
    NeighborIPNotRecognized,
    NeighborIsIPV6,
    ConfiguredNeighborsEmpty,
    NeighborIPNotEstablished,
    NeighborAlreadyEstablished,
    UnableToRemoveNeighbor,
    ConfiguredNeighborNotFound,
    TCPConnDied,
    ASNumMismatch
}

#[derive(PartialEq, Debug)]
pub enum TimerError {
    TimerNotStarted,
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
    MessageHeaderBadLen,
    UpdateMessageLenTooLow,
    HelloTimeLessThanOne,
    HoldTimeLessThanThreeAndNotZero,
    BadAttributeTypeCode,
    UpdateMessageLenAndIdxMismatch,
    MissingWithdrawnRoutes,
    MessageLenTooBig,
    UnableToExtractOptionalParameters
}

#[derive(PartialEq, Debug)]
pub enum EventError {
    UnhandledEvent,
    ChannelDown
}

#[derive(PartialEq, Debug)]
pub enum ProcessError {
    BadNLRILen,
    AS2Unhandled,
    AS4Unhandled,
    ASNumLenMismatch,
}

#[derive(PartialEq, Debug)]
pub enum BGPError {
    Neighbor(NeighborError),
    Message(MessageError),
    Process(ProcessError),
    Timer(TimerError),
    Event(EventError),
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

impl From<EventError> for BGPError {
    fn from(e: EventError) -> BGPError { BGPError::Event(e) }
}

impl From<TimerError> for BGPError {
    fn from(e: TimerError) -> BGPError { BGPError::Timer(e) }
}

// impl From<MessageError> for NeighborError {
//     fn from(e: MessageError) -> NeighborError {NeighborError::Message(e)}
// }
