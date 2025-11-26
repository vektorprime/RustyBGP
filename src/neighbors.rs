use std::net::IpAddr;
use crate::sessions::*;
use crate::finite_state_machine::*;
use crate::timers::*;
use crate::errors::*;
use crate::routes::RouteV4;

pub enum IPType {
    V4,
    V6
}


pub struct Neighbor {
    state: FSM,
    //ip_type: IPType,
    ip: IpAddr,
    hold_time: Timer,
    routes_v4: Vec<RouteV4>,
}

impl Neighbor {
    pub fn new(ip: IpAddr) -> Neighbor {
    //pub fn new(ip: IpAddr, hold_time_sec: u16, keepalive_time_sec: u16) -> Result<Neighbor, NeighborError> {
        // if keepalive_time_sec > hold_time_sec {
        //     return Err(NeighborError::KeepaliveGreaterThanHoldTime)
        // }
        // else if keepalive_time_sec == hold_time_sec {
        //     return Err(NeighborError::KeepaliveEqualToHoldTime)
        // }

        let neighbor = Neighbor {
            state: FSM::default(),
            ip,
            // TODO replace this with better logic
            hold_time: Timer::new(180),
            routes_v4: Vec::new(),
        };

        //Ok(neighbor)
        neighbor
    }
}
