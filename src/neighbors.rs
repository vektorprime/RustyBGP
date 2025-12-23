use std::io;
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
use crate::messages::{extract_messages_from_rec_data, route_incomming_message_to_handler};
use crate::messages::open::get_neighbor_ipv4_address_from_stream;
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
    // TODO make negotiated Option
    pub negotiated_hold_time_sec: u16,
    pub routes_v4: Vec<RouteV4>,
    pub peer_type: PeerType,
    ip_type: IPType,

}

impl Neighbor {

    pub fn set_keepalive_time(&mut self, keepalive_time_sec: u16) -> Result<(), MessageError> {
        if keepalive_time_sec < 1 {
            return Err(MessageError::HelloTimeLessThanOne);
        }
        self.fsm.keepalive_time = keepalive_time_sec;
        Ok(())
    }

    pub fn set_hold_time(&mut self, hold_time_sec: u16) -> Result<(), MessageError> {
        if hold_time_sec < 3 && hold_time_sec != 0 {
            return Err(MessageError::HoldTimeLessThanThreeAndNotZero);
        }
        self.fsm.hold_time = hold_time_sec;
        Ok(())
    }

    pub fn new(ip: Ipv4Addr, as_num: AS, keepalive_time_sec: u16, hold_time_sec: u16, peer_type: PeerType) -> Result<Neighbor, MessageError> {
        //pub fn new(ip: IpAddr, hold_time_sec: u16, keepalive_time_sec: u16) -> Result<Neighbor, NeighborError> {
        // if keepalive_time_sec > hold_time_sec {
        //     return Err(NeighborError::KeepaliveGreaterThanHoldTime)
        // }
        // else if keepalive_time_sec == hold_time_sec {
        //     return Err(NeighborError::KeepaliveEqualToHoldTime)
        // }

        if keepalive_time_sec < 1 {
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
            ip_type: IPType::V4
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

    pub fn handle_event(&mut self, event: Event, tcp_stream: TcpStream) -> Result<(), BGPError> {
        // TODO move the tcp_stream into the neighbor probably in between bgp.run and neighbor.run
        match self.fsm.state {
            State::Idle => {
                // no connections being attempted or accepted
                match event {
                    Event::ManualStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or initial TCP connection here or return result to do it
                        self.fsm.state = State::Connect;
                        Ok(())

                    },
                    Event::AutomaticStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or initial TCP connection here or return result to do it
                        self.fsm.state = State::Connect;
                        Ok(())
                    },
                    Event::ManualStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or return result to do it
                        self.fsm.state = State::Active;
                        Ok(())
                    },
                    Event::AutomaticStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or return result to do it
                        self.fsm.state = State::Active;
                        Ok(())
                    },
                    // ManualStop and AutomaticStop are ignored in Idle
                    // these next 3 events are for preventing peer oscillations
                    Event::AutomaticStartWithDampPeerOscillations => {
                        //TODO use for damp peer oscillations
                        Ok(())
                    },
                    Event::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => {
                        //TODO use for damp peer oscillations
                        Ok(())
                    },
                    Event::IdleHoldTimerExpires => {
                        //TODO use for damp peer oscillations
                        Ok(())
                    },
                    _ => {
                        panic!("Unhandled event in State::Idle")
                    }
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
                        Ok(())
                    },
                    Event::ConnectRetryTimerExpires => {
                        // TODO drop TCP connection, just let it go out of scope
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.delay_open_timer.stop();
                        // TODO continue to listen for connection or try to connect to peer
                        // stay in Connect
                        Ok(())
                    },
                    Event::DelayOpenTimerExpires => {
                        // TODO send Open
                        if self.fsm.hold_time < 240 {
                            self.fsm.hold_timer.start(240);
                        }
                        else {
                            self.fsm.hold_timer.start(self.fsm.hold_time);
                        }
                        // TODO continue to listen for connection and stay in Connect
                        self.fsm.state = State::OpenSent;
                        Ok(())
                    },
                    Event::TcpConnectionValid => {
                        // TODO process the connection
                        Ok(())
                    },
                    Event::TcpCRInvalid => {
                        // TODO reject the TCP connection and remain in Connect
                        Ok(())
                    },
                    Event::TcpCRAcked | Event::TcpConnectionConfirmed => {
                        if self.fsm.delay_open {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.delay_open_timer.start(self.fsm.delay_open_time);
                            // stay in Connect
                        }
                        else {
                            self.fsm.connect_retry_timer.stop();
                            // TODO complete bgp init
                            // TODO send Open
                            if self.fsm.hold_time < 240 {
                                self.fsm.hold_timer.start(240);
                            }
                            else {
                                self.fsm.hold_timer.start(self.fsm.hold_time);
                            }
                            self.fsm.state = State::OpenSent;
                        }
                        Ok(())
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
                        Ok(())
                    },
                    Event::BGPOpenWithDelayOpenTimerRunning => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO complete BGP init?
                        self.fsm.delay_open_timer.stop();
                        // TODO send Open
                        // TODO send Keepalive
                        if self.fsm.hold_time != 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.start(self.negotiated_hold_time_sec);
                        }
                        else {
                            self.fsm.keepalive_timer.stop();
                            self.fsm.hold_timer.stop();
                        }
                        self.fsm.state = State::OpenConfirm;
                        Ok(())
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
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        if self.fsm.delay_open_timer.is_running()? {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.delay_open_timer.stop();
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
                        Ok(())
                    },
                    Event::AutomaticStop | Event::HoldTimerExpires | Event::KeepaliveTimerExpires |
                        Event::IdleHoldTimerExpires | Event::BGPOpen => {

                        self.fsm.connect_retry_timer.stop();
                        self.fsm.delay_open_timer.stop();

                        // TODO drop tcp conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO dampen peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    }
                    _ => {
                        panic!("Unhandled event in State::Connect")
                    }


                }
            },
            State::Active => {
                // listening or trying for new TCP connections
                match event {
                    Event::ManualStop => {
                        if self.fsm.delay_open_timer.is_running()? && self.fsm.send_notification_without_open {
                            // TODO send notification
                        }
                        // TODO release all resources
                        self.fsm.delay_open_timer.stop();
                        // TODO drop TCP connection, just let it go out of scope
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.stop();
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::ConnectRetryTimerExpires => {
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO continue to listen for connection or try to connect to peer
                        self.fsm.state = State::Connect;
                        Ok(())
                    },
                    Event::DelayOpenTimerExpires => {
                        self.fsm.connect_retry_timer.stop();
                        self.fsm.delay_open_timer.stop();

                        // TODO send open

                        if self.fsm.hold_time < 240 {
                            self.fsm.hold_timer.start(240);
                        }
                        else {
                            self.fsm.hold_timer.start(self.fsm.hold_time);
                        }

                        self.fsm.state = State::OpenSent;
                        Ok(())
                    },
                    Event::TcpConnectionValid => {
                        // process the TCP flags and stay in Active
                        Ok(())
                    },
                    Event::TcpCRInvalid => {
                        // reject TCP conn and stay in Active
                        Ok(())
                    },
                    Event::TcpCRAcked | Event::TcpConnectionConfirmed => {
                        if self.fsm.delay_open {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.delay_open_timer.start(self.fsm.delay_open_time);
                            // stay in Active
                        }
                        else {
                            self.fsm.connect_retry_timer.stop();
                            // TODO complete bgp init
                            // TODO send open
                            if self.fsm.hold_time < 240 {
                                self.fsm.hold_timer.start(240);
                            }
                            else {
                                self.fsm.hold_timer.start(self.fsm.hold_time);
                            }
                            self.fsm.state = State::OpenSent;
                        }
                        Ok(())
                    },
                    Event::TcpConnectionFails => {
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        self.fsm.delay_open_timer.stop();
                        // TODO release BGP resources
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO
                        }
                        // TODO drop TCP conn
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::BGPOpenWithDelayOpenTimerRunning => {
                        self.fsm.connect_retry_timer.stop();
                        self.fsm.delay_open_timer.stop();
                        // TODO complete BGP init?
                        // TODO send open
                        // TODO send keepalive
                        if self.fsm.hold_time != 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.start(self.negotiated_hold_time_sec);
                        }
                        else {
                            self.fsm.keepalive_timer.stop();
                            self.fsm.hold_timer.stop();
                        }
                        self.fsm.state = State::OpenConfirm;
                        Ok(())
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
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        if self.fsm.delay_open_timer.is_running()? {
                            self.fsm.connect_retry_timer.stop();
                            self.fsm.delay_open_timer.stop();
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
                        Ok(())
                    },
                    Event::AutomaticStop | Event::HoldTimerExpires | Event::KeepaliveTimerExpires |
                    Event::IdleHoldTimerExpires | Event::BGPOpen | Event::OpenCollisionDump |
                    Event::NotifMsg | Event::KeepAliveMsg | Event::UpdateMsg | Event::UpdateMsgErr => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO drop tcp conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO dampen peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())

                    },
                    _ => {
                        panic!("Unhandled event in State::Active")
                    }

                }
            },
            State::OpenSent => {
                match event {
                    Event::ManualStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::AutomaticStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::HoldTimerExpires => {
                        // TODO send notification with Hold Timer Expired
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::TcpConnectionValid | Event::TcpCRAcked | Event::TcpConnectionConfirmed => {
                        // TODO handle collision here (may have a second TCP conn opening)
                        Ok(())
                    },
                    Event::TcpCRInvalid => {
                        // ignored
                        Ok(())
                    },
                    Event::TcpConnectionFails => {
                        // closes the BGP conn
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // listen for a conn
                        self.fsm.state = State::Active;
                        Ok(())
                    },
                    Event::BGPOpen => {
                        self.fsm.delay_open_timer.stop();
                        self.fsm.connect_retry_timer.stop();
                        // TODO send Keepalive

                        if self.negotiated_hold_time_sec == 0 {
                            self.fsm.hold_timer.stop();
                            self.fsm.keepalive_timer.stop();
                        }
                        else {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.start(self.negotiated_hold_time_sec);
                        }
                        self.fsm.state = State::OpenConfirm;
                        Ok(())
                    },
                    Event::BGPHeaderErr | Event::BGPOpenMsgErr => {
                      // TODO send Notification with error code
                        self.fsm.connect_retry_timer.stop();
                        // release resources
                        // drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())

                    },
                    Event::OpenCollisionDump => {
                        // TODO send Notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // rlease bgp resources
                        // drop tcp conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        self.fsm.connect_retry_timer.stop();
                        // rlease bgp resources
                        // drop tcp conn
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::ConnectRetryTimerExpires | Event::KeepaliveTimerExpires | Event::DelayOpenTimerExpires |
                    Event::IdleHoldTimerExpires | Event::BGPOpenWithDelayOpenTimerRunning | Event::NotifMsg |
                    Event::KeepAliveMsg | Event::UpdateMsg | Event::UpdateMsgErr => {
                        // TODO send Notification with error code FSM error
                        self.fsm.connect_retry_timer.stop();
                        // TODO drop tcp conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO dampen peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())

                    },
                    _ => {
                        panic!("Unhandled event in State::OpenSent")
                    }

                }
            },
            State::OpenConfirm => {
                match event {
                    Event::ManualStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::AutomaticStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::HoldTimerExpires => {
                        // TODO send notification with Hold Timer Expired
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::KeepaliveTimerExpires => {
                        // TODO send Keepalive
                        self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                        // stay in OpenConfirm
                        Ok(())
                    },
                    Event::TcpConnectionValid | Event::TcpCRAcked | Event::TcpConnectionConfirmed => {
                        // TODO
                        Ok(())
                    },
                    Event::TcpCRInvalid => {
                        // ignore second connection attempt
                        Ok(())
                    },
                    Event::TcpConnectionFails | Event::NotifMsg => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::BGPOpen => {
                        // TODO look into this step more
                        // TODO check for collision, if true break down conn
                        // TODO send notification with Cease
                        // IF COLLISION TRUE
                        // invoke OpenCOllisionDump event? Need to make sure it's for right conn

                        // IF COLLISION FALSE
                        // PROCESS OPEN
                        Ok(())
                    },
                    Event::BGPHeaderErr | Event::BGPOpenMsgErr => {
                        // TODO send Notification with error code
                        self.fsm.connect_retry_timer.stop();
                        // release resources
                        // drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::OpenCollisionDump => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn (send fin)
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::KeepAliveMsg => {
                        self.fsm.hold_timer.start(self.fsm.hold_time);
                        self.fsm.state = State::Established;
                        Ok(())
                    },
                    Event::ConnectRetryTimerExpires | Event::DelayOpenTimerExpires | Event::IdleHoldTimerExpires |
                    Event::BGPOpenWithDelayOpenTimerRunning | Event::UpdateMsg | Event::UpdateMsgErr => {
                        // TODO send Notification with code of FSM error
                        self.fsm.connect_retry_timer.stop();
                        // release resources
                        // drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    _ => {
                        panic!("Unhandled event in State::OpenConfirm")
                    }

                }
            },
            State::Established => {
                match event {
                    Event::ManualStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::AutomaticStop => {
                        // TODO send notification with Cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::HoldTimerExpires => {
                        // TODO send notification with error code hold timer expired
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::KeepaliveTimerExpires => {
                        // TODO send Keepalive
                        if self.negotiated_hold_time_sec > 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                        }
                        Ok(())
                    },
                    // TODO restart keepalive timer everytime a keepalive or update msg is sent

                    Event::TcpConnectionValid => {
                        // TODO track the second conn
                        Ok(())
                    },
                    Event::TcpCRInvalid => {
                        // ignore
                        Ok(())
                    },
                    Event::TcpConnectionConfirmed | Event::TcpCRAcked => {
                        // TODO track the second conn until an open is sent
                        Ok(())
                    },
                    Event::BGPOpen => {
                        if self.fsm.collision_detect_established_state {
                            // check for collision
                            // IF COLLISION
                            // invoke OpenCollisionDump event
                        }
                        Ok(())
                    },
                    Event::OpenCollisionDump => {
                        // TODO send Notification with cease
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::TcpConnectionFails | Event::NotifMsgVerErr | Event::NotifMsg => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::KeepAliveMsg => {
                        if self.negotiated_hold_time_sec > 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                        }
                        // stay in Established
                        Ok(())
                    },
                    Event::UpdateMsg => {
                        // TODO process Update
                        if self.negotiated_hold_time_sec > 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                        }
                        // stay in Established
                        Ok(())
                    },
                    Event::UpdateMsgErr => {
                        // TODO send Notification with an update error
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                            // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                   Event::ConnectRetryTimerExpires | Event::DelayOpenTimerExpires | Event::IdleHoldTimerExpires |
                        Event::BGPOpenWithDelayOpenTimerRunning | Event::BGPHeaderErr | Event::BGPOpenMsgErr => {

                        // TODO send Notification with an update error
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        if self.fsm.damp_peer_oscillations {
                        // TODO damp peer
                        }
                        self.fsm.state = State::Idle;
                       Ok(())
                   },
                    _ => {
                        panic!("Unhandled event in State::Established")
                    }
                }
            },
        }
    }
    pub async fn run(&mut self, mut tcp_stream: TcpStream, bgp_proc: Arc<Mutex<BGPProcess>>) {
    //pub async fn run(&mut self, tcp_stream: TcpStream) {
        tokio::spawn(async move {
        // max bgp msg size should never exceed 4096
        // TODO determine if we want to honor the above rule or not, if not, how should we handle it
        let mut tsbuf: Vec<u8> = Vec::with_capacity(65536);
        //let tcp_stream = ts;
        loop {
            tcp_stream.readable().await.unwrap();
            //read tcp stream into buf and save size read
            let ip = match get_neighbor_ipv4_address_from_stream(&tcp_stream) {
                Ok(i) => i,
                Err(_) => {continue}
            };
            match tcp_stream.try_read_buf(&mut tsbuf) {
                Ok(0) => {
                    {
                        let mut bgp = bgp_proc.lock().await;
                        if bgp.is_neighbor_established(ip) {
                            if let Err(e) = bgp.remove_established_neighbor(ip) {
                                println!("ERROR: {:#?}", e);
                            }
                        }
                    }
                },
                Ok(size) => {
                    println!("Read {} bytes from the stream. ", size);
                    let hex = tsbuf[..size]
                        .iter()
                        .map(|b| format!("{:02X} ", b))
                        .collect::<String>();
                    println!("Data read from the stream: {}", hex);
                    // min valid bgp msg len is 19 bytes
                    if tsbuf.len() < 19 {
                        println!("Data in stream too low to be a valid message, skipping: {}", hex);
                        continue;
                    }
                    match extract_messages_from_rec_data(&tsbuf[..size]) {
                        Ok(messages) => {
                            for m in &messages {
                                {
                                    let mut bgp = bgp_proc.lock().await;
                                    match route_incomming_message_to_handler(&mut tcp_stream, m, &mut *bgp).await {
                                        Ok(_) => {},
                                        Err(e) => {
                                            println!("Error handling message: {:#?}", e);
                                        }
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error extracting messages: {:#?}", e);
                        }
                    }
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    } else {
                        println!("Error : {:#?}", e);
                        {
                            let mut bgp = bgp_proc.lock().await;
                            if bgp.is_neighbor_established(ip) {
                                if let Err(e) = bgp.remove_established_neighbor(ip) {
                                    println!("ERROR: {:#?}", e);
                                }
                            }
                        }
                        break;
                        //return Err(e.into());
                    }
                }
                //let size = ts.read(&mut tsbuf[..]).unwrap();
                //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
            }
            tsbuf.clear();
        }
        });
    }

}
