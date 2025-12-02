use std::net::Ipv4Addr;
use crate::messages::update::*;

#[derive(Debug)]
pub enum RouteCast {
    Unicast,
    Multicast,
}


#[derive(Debug)]
pub struct RouteV4 {
    //route_cast: RouteCast,
    // TODO validate prefix len
    prefix: Ipv4Addr,
    len: u8,
    // mandatory
    origin: Origin,
    as_path: AsPath,
    next_hop: Ipv4Addr,
    // required/should for ibgp
    local_pref: Option<LocalPref>,
    // discretionary
    multi_exit_disc: Option<MultiExitDisc>,
    atomic_aggregate: Option<AtomicAggregate>,
    aggregator: Option<Aggregator>,
}

// impl RouteV4 {
//     pub fn new_from_internal
// }