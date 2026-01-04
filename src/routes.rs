use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::errors::{BGPError, ProcessError};
use crate::messages::update::*;

use serde::Deserialize;

#[derive(Debug)]
pub enum RouteCast {
    Unicast,
    Multicast,
}


#[derive(Eq, Hash, PartialEq, Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct NLRI {
    // TODO validate prefix len
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
        bytes.extend(self.prefix.to_bits().to_be_bytes());
        bytes
    }

}

// pub fn convert_configured_networks_to_nlri(Vec<NetAdvertisementsConfig>) ->

impl FromStr for NLRI {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let adv_parts: Vec<&str> = s.split('/').collect();
        if adv_parts.len() != 2 {
            return Err("Unable to parse net_advertisements_config".parse().unwrap());
        }
        let prefix = Ipv4Addr::from_str(adv_parts[0]).map_err(|_| "Unable to parse prefix from net_advertisements_config".to_string())?;
        let prefix_len = u8::from_str(adv_parts[1]).map_err(|_| "Unable to parse prefix len from net_advertisements_config".to_string())?;
        let nlri = NLRI::new(prefix, prefix_len).map_err(|_| "unable to create NLRI from prefix and len in config".to_string())?;
        Ok(nlri)
    }
}

impl TryFrom<String> for NLRI {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let adv_parts: Vec<&str> = s.split('/').collect();
        if adv_parts.len() != 2 {
            return Err("Unable to parse net_advertisements_config".parse().unwrap());
        }
        let prefix = Ipv4Addr::from_str(adv_parts[0]).map_err(|_| "Unable to parse prefix from net_advertisements_config".to_string())?;
        let prefix_len = u8::from_str(adv_parts[1]).map_err(|_| "Unable to parse prefix len from net_advertisements_config".to_string())?;
        let nlri = NLRI::new(prefix, prefix_len).map_err(|_| "unable to create NLRI from prefix and len in config".to_string())?;
        Ok(nlri)
    }
}

#[derive(Debug, Clone)]
pub struct RouteV4 {
    // route_cast: RouteCast,
    // Maybe at some point I'll try handling multicast routes once. It's pretty rare to need BGP's multicast AF.
    // TODO Keep nlri here even though it's the key for the hashmap, it's useful
    pub nlri: NLRI,
    // mandatory
    pub origin: Origin,
    pub as_path: AsPath,
    pub next_hop: NextHop,
    // should be set for ibgp
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