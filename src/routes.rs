use std::net::Ipv4Addr;
use crate::errors::{BGPError, ProcessError};
use crate::messages::update::*;

#[derive(Debug)]
pub enum RouteCast {
    Unicast,
    Multicast,
}


#[derive(PartialEq, Debug, Clone)]
pub struct NLRI {
    pub len: u8,
    // TODO handle bigger prefixes and padding/trailing bits so that this falls on a byte boundary
    pub prefix: Ipv4Addr
}


impl NLRI {
    pub fn new(prefix: Ipv4Addr, len: u8) -> Result<NLRI, ProcessError> {
        if len < 1 || len > 32 {
            return Err(ProcessError::BadNLRILen)
        }
        let nlri = NLRI {
            len,
            prefix
        };

        Ok(nlri)
    }

    pub fn convert_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.len.to_be_bytes());
        bytes.extend(self.prefix.to_bits().to_le_bytes());
        bytes
    }

}

#[derive(Debug)]
pub struct RouteV4 {
    //route_cast: RouteCast,
    // TODO validate prefix len
    pub nlri: NLRI,
    // mandatory
    pub origin: Origin,
    pub as_path: AsPath,
    pub next_hop: NextHop,
    // required/should for ibgp
    pub local_pref: Option<LocalPref>,
    // discretionary
    pub multi_exit_disc: Option<MultiExitDisc>,
    pub atomic_aggregate: Option<AtomicAggregate>,
    pub aggregator: Option<Aggregator>,
}

impl RouteV4 {
    pub fn new(nlri: NLRI, origin: Origin, as_path: AsPath, next_hop: NextHop,
        local_pref: Option<LocalPref>, multi_exit_disc: Option<MultiExitDisc>,
        atomic_aggregate: Option<AtomicAggregate>, aggregator: Option<Aggregator>) -> Self {

         RouteV4 {
            nlri,
            origin,
            as_path,
            next_hop,
            local_pref,
            multi_exit_disc,
            atomic_aggregate,
            aggregator
        }

    }
}