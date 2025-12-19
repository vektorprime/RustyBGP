use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use crate::sessions::*;
use crate::finite_state_machine::*;
use crate::timers::*;
use crate::messages::update::*;
use crate::errors::*;
use crate::errors::BGPError::Message;
use crate::routes::RouteV4;
use crate:: finite_state_machine::events::*;
use crate:: finite_state_machine::session_attributes::*;
use crate::process::BGPProcess;

#[derive(Debug)]
pub enum IPType {
    V4,
    V6
}

#[derive(Debug)]
pub enum PeerType {
    Internal,
    External
}

#[derive(Debug)]
pub struct Neighbor {
    pub fsm: FSM,
    //ip_type: IPType,
    pub ip: Ipv4Addr,
    pub as_num: AS,
    // moved these timers to FSM struct
    //pub hello_time_sec: u16,
    //pub hold_time_sec: u16,
    //pub hold_timer: Timer,
    pub negotiated_hold_time_sec: u16,
    pub routes_v4: Vec<RouteV4>,
    pub peer_type: PeerType,

}

impl Neighbor {

    pub fn new(ip: Ipv4Addr, as_num: AS, hello_time_sec: u16, hold_time_sec: u16, peer_type: PeerType) -> Result<Neighbor, MessageError> {
        //pub fn new(ip: IpAddr, hold_time_sec: u16, keepalive_time_sec: u16) -> Result<Neighbor, NeighborError> {
        // if keepalive_time_sec > hold_time_sec {
        //     return Err(NeighborError::KeepaliveGreaterThanHoldTime)
        // }
        // else if keepalive_time_sec == hold_time_sec {
        //     return Err(NeighborError::KeepaliveEqualToHoldTime)
        // }

        if hello_time_sec < 1 {
            return Err(MessageError::HelloTimeLessThanOne);
        }

        if hold_time_sec < 3 && hold_time_sec != 0 {
            return Err(MessageError::HoldTimeLessThanThreeAndNotZero);
        }


        Ok(Neighbor {
            fsm: FSM::default(),
            ip,
            as_num,
            negotiated_hold_time_sec: hold_time_sec,
            routes_v4: Vec::new(),
            peer_type,
        })

    }

    pub fn process_routes_from_message(&mut self, update_message: UpdateMessage) -> Result<(), MessageError> {


        if let Some(nlri_coll) = update_message.nlri {
            let path_attributes = update_message.path_attributes.ok_or_else(|| MessageError::MissingPathAttributes)?;
            //let needed_pa_list: Vec<TypeCode> = vec![TypeCode::Origin, TypeCode::AsPath, TypeCode::NextHop, TypeCode::LocalPref, TypeCode::MultiExitDisc, TypeCode::AtomicAggregate, TypeCode::Aggregator];
            let origin = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::Origin, &path_attributes).ok_or_else(|| MessageError::MissingPathAttributes)?;
                let PAdata::Origin(origin) = data else {
                    return Err(MessageError::MissingPathAttributes)
                };
                origin
            };
            let as_path = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::AsPath, &path_attributes).ok_or_else(|| MessageError::MissingPathAttributes)?;
                let PAdata::AsPath(as_path) = data else {
                    return Err(MessageError::MissingPathAttributes)
                };
                as_path
            };
            let next_hop = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::NextHop, &path_attributes).ok_or_else(|| MessageError::MissingPathAttributes)?;
                let PAdata::NextHop(next_hop) = data else {
                    return Err(MessageError::MissingPathAttributes)
                };
                next_hop
            };
            let local_pref: Option<LocalPref> = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::LocalPref, &path_attributes);
                if let Some(PAdata::LocalPref(local_pref)) = data {
                    Some(local_pref)
                } else {
                    None
                }
            };
            let med: Option<MultiExitDisc> = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::MultiExitDisc, &path_attributes);
                if let Some(PAdata::MultiExitDisc(med)) = data {
                    Some(med)
                } else {
                    None
                }
            };
            let atomic_agg: Option<AtomicAggregate> = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::AtomicAggregate, &path_attributes);
                if let Some(PAdata::AtomicAggregate(atomic_agg)) = data {
                    Some(atomic_agg)
                } else {
                    None
                }
            };
            let agg: Option<Aggregator> = {
                let data = PathAttribute::get_pa_data_from_pa_vec(TypeCode::Aggregator, &path_attributes);
                if let Some(PAdata::Aggregator(agg)) = data {
                    Some(agg)
                } else {
                    None
                }
            };

            // let as_path = PathAttribute::get_pa_data_from_pa_vec(TypeCode::AsPath, &path_attributes).ok_or_else(|| MessageError::MissingPathAttributes)?;
            // let next_hop = PathAttribute::get_pa_data_from_pa_vec(TypeCode::NextHop, &path_attributes).ok_or_else(|| MessageError::MissingPathAttributes)?;
            // let local_pref = PathAttribute::get_pa_data_from_pa_vec(TypeCode::LocalPref, &path_attributes);
            // let multi_exit_disc = PathAttribute::get_pa_data_from_pa_vec(TypeCode::MultiExitDisc, &path_attributes);
            // let atomic_aggregate = PathAttribute::get_pa_data_from_pa_vec(TypeCode::AtomicAggregate, &path_attributes);
            // let aggregator = PathAttribute::get_pa_data_from_pa_vec(TypeCode::Aggregator, &path_attributes);
            for nlri in &nlri_coll {
                // debating if I should do the checks here or move more logic into new()
                let rt = RouteV4::new(nlri.clone(), origin.clone(), as_path.clone(), next_hop.clone(), local_pref.clone(), med.clone(), atomic_agg.clone(), agg.clone() );
                println!("Adding Route {:#?} to vec", rt);
                self.routes_v4.push(rt);
            }
            Ok(())
        }
        else {
            Err(MessageError::MissingNLRI)
        }


    }

    pub fn handle_event(&mut self, event: Event, tcp_stream: TcpStream) {
        // TODO move the tcp_stream into the neighbor probably in between bgp.run and neighbor.run
        match self.fsm.state {
            State::Idle => {
                // no connections being attempted or accepted
                match event {
                    Event::ManualStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.state = State::Connect;
                        // TODO start TCP listener or initial TCP connection here or return result to do it

                    },
                    Event::AutomaticStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.state = State::Connect;
                        // TODO start TCP listener or initial TCP connection here or return result to do it

                    },
                    Event::ManualStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.state = State::Active;
                        // TODO start TCP listener or return result to do it

                    },
                    Event::AutomaticStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.state = State::Active;
                        // TODO start TCP listener or return result to do it
                    },
                    // ManualStop and AutomaticStop are ignored in Idle
                    // these next 3 events are for preventing peer oscillations
                    Event::AutomaticStartWithDampPeerOscillations => {
                        //TODO handle
                    },
                    Event::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => {
                        //TODO handle
                    },
                    Event::IdleHoldTimerExpires => {
                        //TODO handle
                    },
                }
            },
            State::Connect => {
                // initiates and/or waits for the TCP connection to complete
                match event {
                    Event::ManualStop => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.stop();
                        // TODO drop TCP connection, just let it go out of scope
                        self.fsm.state = State::Idle;
                    },
                    Event::ConnectRetryTimerExpires => {
                        // TODO drop TCP connection, just let it go out of scope
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.delay_open_timer.stop();
                        // TODO continue to listen for connection or try to connect to peer, and stay in Connect
                    },
                    Event::DelayOpenTimerExpires => {
                        // TODO send open

                        if self.fsm.idle_hold_time + 30 < 65505 {
                            self.fsm.idle_hold_time += 30;
                        }
                        self.fsm.idle_hold_timer.start(self.fsm.idle_hold_time);

                        // TODO continue to listen for connection and stay in Connect

                            self.fsm.state = State::OpenSent;
                    },
                    Event::TcpConnectionValid => {
                        // TODO
                    },
                    Event::TcpCRInvalid => {
                        // TODO
                    },
                    Event::TcpCRAcked | Event::TcpConnectionConfirmed => {

                        if self.fsm.delay_open {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.delay_open_timer.start(self.fsm.delay_open_time);
                        }
                        else {
                            self.fsm.connect_retry_timer.stop();
                            // TODO complete bgp init
                            // TODO send open
                            // set the HoldTimer to a large value (why after?)
                            self.fsm.state = State::OpenSent;
                        }

                    },
                    Event::TcpConnectionFails => {

                        if self.fsm.delay_open_timer.is_running()? {
                            self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                            self.fsm.delay_open_timer.stop();
                            // TODO continue to listen for TCP conns
                            self.fsm.state = State::Active;
                        }
                        else {
                            self.fsm.connect_retry_timer.stop();
                            // TODO drop TCP conn
                            self.fsm.state = State::Idle;
                        }

                    },
                    Event::BGPOpenWithDelayOpenTimerRunning => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO complete BGP init?
                        self.fsm.delay_open_timer.stop();
                        // TODO send open and keepalive
                        if self.fsm.hold_time != 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.start(self.negotiated_hold_time_sec);
                        }
                        else {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.stop();
                        }
                        self.fsm.state = State::OpenConfirm;
                    },
                    Event::BGPHeaderErr | Event::BGPOpenMsgErr => {
                        if self.fsm.send_notification_without_open {
                            // TODO send notification
                        }
                        self.fsm.connect_retry_timer.stop();
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO dampen peer
                        }
                        self.fsm.state = State::Idle;
                    },
                    Event::NotifMsgVerErr => {
                        if self.fsm.delay_open_timer.is_running()? {
                            self.fsm.connect_retry_timer.stop();
                            // TODO drop tcp conn
                        }
                        else {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.connect_retry_counter += 1;
                            if self.fsm.damp_peer_oscillations {
                                // TODO dampen peer
                            }
                        }
                        self.fsm.state = State::Idle;
                    },
                    Event::AutomaticStop | Event::HoldTimerExpires | Event::KeepaliveTimerExpires |
                        Event::IdleHoldTimerExpires | Event::BGPOpen => {

                        if self.fsm.connect_retry_timer.is_running()? {
                            self.fsm.connect_retry_timer.stop();
                        }
                        if self.fsm.delay_open_timer.is_running()? {
                            self.fsm.delay_open_timer.stop();
                        }
                        // TODO drop tcp conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO dampen peer
                        }
                        self.fsm.state = State::Idle;

                    }


                }
            },
            State::Active => {
                // listening or trying for new TCP connections
                match event {

                }
            },
            State::OpenSent => {
                match event {

                }
            },
            State::OpenConfirm => {
                match event {

                }
            },
            State::Established => {
                match event {

                }
            },

        }
    }

}
