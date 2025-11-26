
#[derive(Debug)]
pub enum NeighborError {
    KeepaliveGreaterThanHoldTime,
    KeepaliveEqualToHoldTime,
}

pub enum MessageError {
    NoMarkerFound,
    NoLenFound,
}

#[derive(Debug)]
pub enum BGPError {
    Neighbor(NeighborError),
}
impl From<NeighborError> for BGPError {
    fn from(e: NeighborError) -> BGPError {
        BGPError::Neighbor(e)
    }
}