use std::net::IpAddr;
use crate::sessions::*;
use crate::finite_state_machine::*;
use crate::timers::*;
use crate::messages::update::AS;
use crate::errors::*;
use crate::routes::RouteV4;

pub enum IPType {
    V4,
    V6
}



pub struct Neighbor {
    pub state: FSM,
    //ip_type: IPType,
    pub ip: IpAddr,
    pub as_num: AS,
    pub hello_time: u16,
    pub hold_time: Option<Timer>,
    pub routes_v4: Vec<RouteV4>,
}

impl Neighbor {
    pub fn new(ip: IpAddr, as_num: AS, hello_time_sec: u16, hold_time_sec: u16) -> Neighbor {
    //pub fn new(ip: IpAddr, hold_time_sec: u16, keepalive_time_sec: u16) -> Result<Neighbor, NeighborError> {
        // if keepalive_time_sec > hold_time_sec {
        //     return Err(NeighborError::KeepaliveGreaterThanHoldTime)
        // }
        // else if keepalive_time_sec == hold_time_sec {
        //     return Err(NeighborError::KeepaliveEqualToHoldTime)
        // }
        if hold_time_sec == 0 {
           Neighbor {
                state: FSM::default(),
                ip,
                as_num,
                // TODO replace this with better logic
                hello_time: hello_time_sec,
                hold_time: None,
                routes_v4: Vec::new(),
            }
        } else {
             Neighbor {
                state: FSM::default(),
                ip,
                as_num,
                // TODO replace this with better logic
                hello_time: hello_time_sec,
                hold_time: Some(Timer::new(hold_time_sec)),
                routes_v4: Vec::new(),
            }
        }

    }
}
