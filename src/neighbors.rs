use std::collections::{HashMap, VecDeque};
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
use crate::errors::EventError::UnhandledEvent;
use crate::routes::RouteV4;
use crate::finite_state_machine::events::Event;

use crate::messages::{extract_messages_from_rec_data, parse_packet_type, BGPVersion, MessageType};
use crate::messages::keepalive::{handle_keepalive_message, send_keepalive};
use crate::messages::open::{extract_open_message, get_neighbor_ipv4_address_from_stream, send_open, test_net_advertisements, OpenMessage};
use crate::process::{BGPProcess, GlobalSettings};

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
    pub ip_type: IPType,
    pub global_settings: GlobalSettings,
    pub events: VecDeque<Event>,
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

    pub fn new(ip: Ipv4Addr, as_num: AS, keepalive_time_sec: u16, hold_time_sec: u16, peer_type: PeerType, settings: &GlobalSettings) -> Result<Neighbor, MessageError> {
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
            ip_type: IPType::V4,
            global_settings: settings.clone(), // we don't store a reference here because it gets too complicated
            // we'll update all neighbor from the BGP proc settings when anything changes.
            events: VecDeque::new(),
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

    pub async fn handle_event(&mut self, event: Event, tcp_stream: &mut TcpStream) -> Result<(), BGPError> {
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
                        // We are already listening for TCP connections so we should just go straight to active
                        self.fsm.state = State::Active;
                        Ok(())
                    },
                    Event::AutomaticStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // We are already listening for TCP connections so we should just go straight to active
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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
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
                        Event::IdleHoldTimerExpires | Event::OpenMsg(_) => {

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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
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
                            let open_message = OpenMessage::new(self.global_settings.version, self.global_settings.my_as, self.fsm.hold_time, self.global_settings.identifier, 0, None)?;
                            send_open(tcp_stream, open_message).await?;
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
                    Event::IdleHoldTimerExpires | Event::OpenMsg(_) | Event::OpenCollisionDump |
                    Event::NotifMsg(_) | Event::KeepAliveMsg | Event::UpdateMsg(_) | Event::RouteRefreshMsg(_) | Event::UpdateMsgErr => {
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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
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
                    Event::OpenMsg(msg) => {
                        self.fsm.delay_open_timer.stop();
                        self.fsm.connect_retry_timer.stop();
                        send_keepalive(tcp_stream).await?;
                        // TODO process Open (anything left?)
                        if msg.hold_time < self.fsm.hold_time {
                            self.negotiated_hold_time_sec = msg.hold_time;
                        }
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
                    Event::IdleHoldTimerExpires | Event::BGPOpenWithDelayOpenTimerRunning | Event::NotifMsg(_) | Event::RouteRefreshMsg(_) |
                    Event::KeepAliveMsg | Event::UpdateMsg(_) | Event::UpdateMsgErr => {
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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
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
                    Event::TcpConnectionFails | Event::NotifMsg(_) => {
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
                    Event::OpenMsg(msg) => {
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
                    Event::BGPOpenWithDelayOpenTimerRunning | Event::UpdateMsg(_) | Event::UpdateMsgErr => {
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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
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
                    Event::OpenMsg(msg) => {
                        // TODO handle this
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
                    Event::NotifMsg(msg) => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        self.fsm.state = State::Idle;
                        Ok(())
                    },
                    Event::TcpConnectionFails | Event::NotifMsgVerErr  => {
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
                    Event::UpdateMsg(msg) => {
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
                    Event::RouteRefreshMsg(msg) => {
                        // TODO handle route refresh
                        Ok(())
                    }
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
                        println!("Unhandled event in {:?} - {:#?}",self.fsm.state, event);
                        Err(EventError::UnhandledEvent.into())
                    }
                }
            },
        }
    }



    pub async fn handle_open_message(&mut self, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), BGPError> {
        // TODO handle better, for now just accept the neighbor and mirror the capabilities for testing
        // compare the open message params to configured neighbor
        let received_open = extract_open_message(tsbuf)?;

        //let peer_ip = get_neighbor_ipv4_address_from_stream(tcp_stream)?;

        if self.is_established() {
            return Err(NeighborError::NeighborAlreadyEstablished.into())
        }


        //let neighbor = bgp_proc.neighbors.get_mut(&peer_ip).ok_or_else(|| NeighborError::PeerIPNotRecognized)?;
        let open_message = OpenMessage::new(BGPVersion::V4, bgp_proc.global_settings.my_as, self.fsm.hold_time, bgp_proc.global_settings.identifier, 0, None)?;
        send_open(tcp_stream, open_message).await?;
        send_keepalive(tcp_stream).await?;
        //TODO handle this error so it doesn't return
        self.set_hold_time(received_open.hold_time)?;
        self.fsm.state = State::Established;

        //add_neighbor_from_message(bgp_proc, &mut received_open, peer_ip, cn.hello_time, cn.hold_time)?;

        // TODO REMOVE AFTER TESTING IT WORKS HERE
        test_net_advertisements(bgp_proc, tcp_stream).await?;

        Ok(())

    }

    pub async fn handle_update_message(&mut self, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), BGPError> {
        println!("Handling update message");
        if !self.is_established() {
            return Err(NeighborError::PeerIPNotEstablished.into())
        }
        let update_message = extract_update_message(tsbuf)?;
        self.process_routes_from_message(update_message)?;

        Ok(())
    }


    pub async fn route_incoming_message_to_handler(&mut self, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), BGPError> {
        let message_type = parse_packet_type(&tsbuf)?;
        println!("{}", message_type);
        match message_type {
            MessageType::Open => {
                self.handle_open_message(tcp_stream, tsbuf, bgp_proc).await?
            },
            MessageType::Update => {
                self.handle_update_message(tcp_stream, tsbuf, bgp_proc).await?
            },
            MessageType::Notification => {
                crate::messages::notification::handle_notification_message(tcp_stream, tsbuf, bgp_proc).await?;
            },
            MessageType::Keepalive => {
                handle_keepalive_message(tcp_stream).await?
            },
            MessageType::RouteRefresh => {
                // TODO handle route refresh
                crate::messages::route_refresh::handle_route_refresh_message(tcp_stream, tsbuf)?
            }
        }
        Ok(())
    }

    pub fn is_established(&self) -> bool {
        if self.fsm.state == State::Established { true }
        else { false }
    }

    pub fn check_timers_and_generate_events(&mut self) {
        if let Ok(true) = self.fsm.connect_retry_timer.is_elapsed() {
           self.events.push_back(Event::ConnectRetryTimerExpires);
        }
        if let Ok(true) = self.fsm.hold_timer.is_elapsed() {
            self.events.push_back(Event::HoldTimerExpires);
        }
        if let Ok(true) = self.fsm.keepalive_timer.is_elapsed() {
            self.events.push_back(Event::KeepaliveTimerExpires);
        }
        if let Ok(true) = self.fsm.delay_open_timer.is_elapsed() {
            self.events.push_back(Event::DelayOpenTimerExpires);
        }
        if let Ok(true) = self.fsm.idle_hold_timer.is_elapsed() {
            self.events.push_back(Event::IdleHoldTimerExpires);
        }
    }

    pub fn generate_event(&mut self, event: Event) {
        self.events.push_back(event);
    }

    pub fn generate_event_from_message(&mut self, tsbuf: &Vec<u8>, message_type: MessageType) -> Result<(), BGPError> {
        println!("{}", message_type);
        match message_type {
            MessageType::Open => {
                let received_msg = extract_open_message(tsbuf)?;
                self.events.push_back(Event::OpenMsg(received_msg));
            },
            MessageType::Update => {
                let received_msg = extract_update_message(tsbuf)?;
                self.events.push_back(Event::UpdateMsg(received_msg));
            },
            MessageType::Notification => {
                // TODO create the func
                // let received_msg = extract_notification_message(tsbuf)?;
                //self.events.push_back(Event::NotifMsg(received_msg));
            },
            MessageType::Keepalive => {
                self.events.push_back(Event::KeepAliveMsg);
            },
            MessageType::RouteRefresh => {
                // TODO create the func
                //let received_msg = extract_route_refresh_message(tsbuf)?;
                //self.events.push_back(Event::RouteRefreshMsg(received_msg));
            }
        }
        Ok(())
    }
}

//
// pub async fn unlock_arc() {
//
// }



pub async fn run(mut tcp_stream: TcpStream, bgp_proc: Arc<Mutex<BGPProcess>>, all_neighbors: Arc<Mutex<HashMap<Ipv4Addr, Neighbor>>>, peer_ip: Ipv4Addr ) -> Result<(), BGPError> {
    //pub async fn run(&mut self, tcp_stream: TcpStream) {

    // max bgp msg size should never exceed 4096
    // TODO determine if we want to honor the above rule or not, if not, how should we handle it
    let mut tsbuf: Vec<u8> = Vec::with_capacity(65536);
    //let tcp_stream = ts;
    loop {
        tsbuf.clear();
        //TODO check all timers and generate events as needed here
        {
            let mut all_neighbors = all_neighbors.lock().await;
            if let Some(n) = all_neighbors.get_mut(&peer_ip) {
                n.check_timers_and_generate_events();
                while let Some(e) = n.events.pop_front() {
                    if let Err(e) = n.handle_event(e, &mut tcp_stream).await {
                        println!("Error: Unable to handle event {:#?}, skipping", e);
                    }
                }
            }
        }

        tcp_stream.readable().await.unwrap();

        match tcp_stream.try_read_buf(&mut tsbuf) {
            Ok(0) => {
                {
                    let mut all_neighbors = all_neighbors.lock().await;
                    if let Some(n) = all_neighbors.get_mut(&peer_ip) {
                        n.generate_event(Event::TcpConnectionFails);
                        // I should generate this event regardless because at this point we may or may not be established
                        // if n.is_established() {
                        // }
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
                                let mut all_neighbors = all_neighbors.lock().await;
                                if let Some(n) = all_neighbors.get_mut(&peer_ip) {
                                    // if let Err(e) =  n.route_incoming_message_to_handler(&mut tcp_stream, m, &mut *bgp).await {
                                    //     println!("Error handling message: {:#?}", e);
                                    // }
                                   if let Err(e) =  parse_packet_type(m).and_then(|mt| n.generate_event_from_message(&tsbuf, mt)) {
                                       println!("Error: {:#?}, skipping message", e);
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
                }
                else {
                    println!("Error : Unable to use TCP Stream -  {:#?}", e);
                    //return Err(NeighborError::TCPConnDied.into());
                    {
                        let mut all_neighbors = all_neighbors.lock().await;
                        if let Some(n) = all_neighbors.get_mut(&peer_ip) {
                            n.generate_event(Event::TcpConnectionFails);

                        }
                    }
                }
            }
            //let size = ts.read(&mut tsbuf[..]).unwrap();
            //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
        }

    }
}
