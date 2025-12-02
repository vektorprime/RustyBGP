use std::net::{IpAddr, Ipv4Addr};


use crate::sessions::*;
use crate::finite_state_machine::*;
use crate::timers::*;
use crate::messages::update::{UpdateMessage, AS};
use crate::errors::*;
use crate::routes::RouteV4;



#[derive(Debug)]
pub enum IPType {
    V4,
    V6
}


#[derive(Debug)]
pub struct Neighbor {
    pub state: FSM,
    //ip_type: IPType,
    pub ip: Ipv4Addr,
    pub as_num: AS,
    pub hello_time: u16,
    pub hold_time: Option<Timer>,
    pub routes_v4: Vec<RouteV4>,
}

impl Neighbor {

    pub fn process_routes_from_message(&mut self, update_message: UpdateMessage) -> Result<(), MessageError> {


        if let Some(nlri_coll) = update_message.nlri {
            for nlri in &nlri_coll {
                
            }
            Ok(())
        }
        else {
            Err(MessageError::MissingNLRI)

        }


    }
    pub fn new(ip: Ipv4Addr, as_num: AS, hello_time_sec: u16, hold_time_sec: u16) -> Neighbor {
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
