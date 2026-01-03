use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use default::default;
use tokio::net::TcpStream;
use tokio::net::tcp::*;
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::sessions::*;
use crate::finite_state_machine::*;
use crate::timers::*;
use crate::messages::update::*;
use crate::errors::*;
use crate::errors::BGPError::Message;
use crate::errors::EventError::UnhandledEvent;
use crate::routes::{RouteV4, NLRI};
use crate::finite_state_machine::events::Event;

use crate::messages::{extract_messages_from_rec_data, parse_packet_type, BGPVersion, MessageType};
use crate::messages::keepalive::{send_keepalive};
use crate::messages::open::{extract_open_message, get_neighbor_ipv4_address_from_stream, send_open, OpenMessage};
use crate::process::{BGPProcess, ChannelMessage, GlobalSettings, NeighborChannel};

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
    //pub negotiated_hold_time_sec: u16,
    //pub routes_v4: Vec<RouteV4>,
    pub peer_type: PeerType,
    pub ip_type: IPType,
    pub global_settings: GlobalSettings,
    pub events: VecDeque<Event>,
    // TODO adj-rib-in filters routes coming in, then generates events to loc-rib with the route, trigger best path calc here
    pub adj_rib_in: HashMap<NLRI, Vec<RouteV4>>,
    // TODO adj-rib-out filters routes before they are sent to neighbor, generate event to send update
    pub adj_rib_out: HashMap<NLRI, Vec<RouteV4>>,
    pub channel: NeighborChannel,
    //pub tcp_read_stream: Option<OwnedReadHalf>,
    pub tcp_write_stream: Option<OwnedWriteHalf>,
}


pub async fn run_timer_loop(neighbor_arc: Arc<Mutex<Neighbor>>, peer_ip: Ipv4Addr) {
    tokio::spawn( async move {
        loop {
            {
                let mut neighbor = neighbor_arc.lock().await;
                neighbor.check_timers_and_generate_events().await;
            }
            // sleep outside of this so we don't hold a mutex
            sleep(Duration::from_secs(1)).await;
        }
    });
}

pub async fn run_event_loop(neighbor_arc: Arc<Mutex<Neighbor>>) {
    tokio::spawn( async move {
        loop {
            let mut neighbor = neighbor_arc.lock().await;
            while let Some(e) = neighbor.events.pop_front() {
                if let Err(e) = neighbor.handle_event(e).await {
                    println!("Error: Unable to handle event {:#?}, skipping", e);
                }
            }
        }
    });
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

    pub fn new(ip: Ipv4Addr, as_num: AS, keepalive_time_sec: u16, hold_time_sec: u16, peer_type: PeerType, settings: GlobalSettings, route_channel: NeighborChannel) -> Result<Neighbor, MessageError> {
        if keepalive_time_sec < 1 {
            return Err(MessageError::HelloTimeLessThanOne);
        }

        if hold_time_sec < 3 && hold_time_sec != 0 {
            return Err(MessageError::HoldTimeLessThanThreeAndNotZero);
        }

        let fsm = FSM {
            hold_time: hold_time_sec,
            ..default()
        };

        Ok(Neighbor {
            fsm: FSM::default(),
            ip,
            as_num,
            //routes_v4: Vec::new(),
            peer_type,
            ip_type: IPType::V4,
            global_settings: settings, // we don't store a reference here because it gets too complicated
            // we'll update all neighbor from the BGP proc settings when anything changes.
            events: VecDeque::new(),
            adj_rib_in: HashMap::new(),
            adj_rib_out: HashMap::new(),
            channel: route_channel,
            //tcp_read_stream: None,
            tcp_write_stream: None,
        })
    }

    pub async fn withdraw_routes_from_message(&mut self, update_message: UpdateMessage) -> Result<(), MessageError> {

        if let Some(withdrawn_routes) = update_message.withdrawn_routes {
            for nlri in &withdrawn_routes {
                println!("Withdrawing route {:#?} from adj_rib_in", nlri);
                self.adj_rib_in.remove(nlri);
                // TODO get rid of this and handle it better, for now I just want to see the routes coming to the BGP proc loc_rib
            }
            
            self.channel.withdraw_route(withdrawn_routes).await;
            
            Ok(())
        }
        else {
            Err(MessageError::MissingWithdrawnRoutes)
        }
    }

    pub async fn process_routes_from_message(&mut self, update_message: UpdateMessage) -> Result<(), MessageError> {

        if let Some(nlri_coll) = update_message.nlri {
            let path_attributes = update_message.path_attributes.ok_or_else(|| MessageError::MissingPathAttributes)?;
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
                let rt = RouteV4::new(nlri.clone(), origin.clone(), as_path.clone(), next_hop.clone(), local_pref.clone(), med.clone(), atomic_agg.clone(), agg.clone());
                println!("Adding Route {:#?} to adj_rib_in", rt);
                //self.routes_v4.push(rt);
                let route_vec = vec![rt.clone()];
                self.adj_rib_in.insert(nlri.clone(), route_vec);
                // TODO get rid of this and handle it better, for now I just want to see the routes coming to the BGP proc loc_rib
                self.channel.send_route(rt).await;
            }
            Ok(())
        }
        else {
            Err(MessageError::MissingNLRI)
        }
    }

    pub async fn handle_event(&mut self, event: Event) -> Result<(), BGPError> {
        // TODO move the tcp_stream into the neighbor probably in between bgp.run and neighbor.run
        println!("Handling event {:#?} in state {:#?}", event, self.fsm.state);
        match self.fsm.state {
            State::Idle => {
                // no connections being attempted or accepted
                match event {
                    Event::ManualStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or initial TCP connection here or return result to do it
                        self.fsm.state = State::Connect;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())

                    },
                    Event::AutomaticStart => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO start TCP listener or initial TCP connection here or return result to do it
                        self.fsm.state = State::Connect;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::ManualStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // We are already listening for TCP connections so we should just go straight to active
                        self.fsm.state = State::Active;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::AutomaticStartWithPassiveTcpEstablishment => {
                        self.fsm.connect_retry_counter = 0;
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // We are already listening for TCP connections so we should just go straight to active
                        self.fsm.state = State::Active;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        //TODO use for damp peer oscillations and restarting from IDLE
                        self.fsm.idle_hold_timer.stop();
                        self.generate_event(Event::AutomaticStartWithPassiveTcpEstablishment);
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
                            if !self.fsm.passive_tcp_establishment {
                                self.fsm.idle_hold_timer.start(self.fsm.idle_hold_time);
                            }

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
                            self.fsm.hold_timer.start(self.fsm.hold_time);
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
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::ConnectRetryTimerExpires => {
                        self.fsm.connect_retry_timer.start(self.fsm.connect_retry_time);
                        // TODO continue to listen for connection or try to connect to peer
                        self.fsm.state = State::Connect;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                            match self.tcp_write_stream.as_mut() {
                                Some(tcp_write_stream) => {
                                    send_open(tcp_write_stream, open_message).await?;
                                },
                                None => {
                                    println!("Unable to use Neighbor's tcp_write_stream");
                                }
                            }

                            if self.fsm.hold_time < 240 {
                                self.fsm.hold_timer.start(240);
                            }
                            else {
                                self.fsm.hold_timer.start(self.fsm.hold_time);
                            }
                            self.fsm.state = State::OpenSent;
                            println!("Moving to {:#?}", self.fsm.state);
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
                        if !self.fsm.passive_tcp_establishment {
                            self.fsm.idle_hold_timer.start(self.fsm.idle_hold_time);
                        }
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                            self.fsm.hold_timer.start(self.fsm.hold_time);
                        }
                        else {
                            self.fsm.keepalive_timer.stop();
                            self.fsm.hold_timer.stop();
                        }
                        self.fsm.state = State::OpenConfirm;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::OpenMsg(msg) => {
                        self.fsm.delay_open_timer.stop();
                        self.fsm.connect_retry_timer.stop();
                        match self.tcp_write_stream.as_mut() {
                            Some(tcp_write_stream) => {
                                send_keepalive(tcp_write_stream).await?;
                            },
                            None => {
                                println!("Unable to use Neighbor's tcp_write_stream");
                            }
                        }
                        // TODO process Open (anything left?)
                        if msg.hold_time < self.fsm.hold_time {
                            self.fsm.hold_time = msg.hold_time;
                        }
                        if self.fsm.hold_time == 0 {
                            self.fsm.hold_timer.stop();
                            self.fsm.keepalive_timer.stop();
                        }
                        else {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                            self.fsm.hold_timer.start(self.fsm.hold_time);
                        }
                        self.fsm.state = State::OpenConfirm;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        self.fsm.connect_retry_timer.stop();
                        // rlease bgp resources
                        // drop tcp conn
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        if !self.fsm.passive_tcp_establishment {
                            self.fsm.idle_hold_timer.start(self.fsm.idle_hold_time);
                        }
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::NotifMsgVerErr => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::KeepAliveMsg => {
                        self.fsm.hold_timer.start(self.fsm.hold_time);
                        println!("Setting neighbor {:#?} state to Established", self.ip);
                        // TODO Need to confirm that we will always receive a Keepalive on neighbor coming up even if holdtime is 0
                        self.fsm.state = State::Established;
                        println!("Moving to {:#?}", self.fsm.state);
                        //self.channel.bring_up().await?;
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
                        println!("Moving to {:#?}", self.fsm.state);
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
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::KeepaliveTimerExpires => {
                        match self.tcp_write_stream.as_mut() {
                            Some(tcp_write_stream) => {
                                send_keepalive(tcp_write_stream).await?;
                            },
                            None => {
                                println!("Unable to use Neighbor's tcp_write_stream");
                            }
                        }
                        if self.fsm.hold_time > 0 {
                            self.fsm.keepalive_timer.start(self.fsm.keepalive_time);
                        } else {
                            println!("hold_time is 0, skipping Keepalive");
                        }
                        Ok(())
                    },
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
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::NotifMsg(msg) => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::TcpConnectionFails | Event::NotifMsgVerErr  => {
                        self.fsm.connect_retry_timer.stop();
                        // TODO delete all routes for this conn.
                        // TODO release BGP res.
                        // TODO drop TCP conn
                        self.fsm.connect_retry_counter += 1;
                        // self.channel.take_down().await?;
                        if !self.fsm.passive_tcp_establishment {
                            self.fsm.idle_hold_timer.start(self.fsm.idle_hold_time);
                        }
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
                        Ok(())
                    },
                    Event::KeepAliveMsg => {
                        if self.fsm.hold_time > 0 {
                            self.fsm.hold_timer.start(self.fsm.hold_time);
                        } else {
                            println!("Received Keepalive, but our hold_time is 0, skipping Keepalive");
                        }
                        // stay in Established
                        Ok(())
                    },
                    Event::UpdateMsg(msg) => {
                        // TODO filter routes or modify them in the adj_rib_in  here
                        // TODO handle withdrawn routes here too
                        if msg.nlri.is_none() && msg.withdrawn_routes.is_some() {
                            self.withdraw_routes_from_message(msg).await?
                        }
                        else if msg.nlri.is_some() {
                            self.process_routes_from_message(msg).await?;
                        }
                        else {
                            println!("WARNING: msg.nlri and msg.withdrawn_routes are both None in Event::UpdateMsg, likely a parsing issue or fuzzing");
                        }

                        if self.fsm.hold_time > 0 {
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
                        // self.channel.take_down().await?;
                        self.fsm.keepalive_timer.stop();
                        self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                        println!("Moving to {:#?}", self.fsm.state);
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
                       // self.channel.take_down().await?;
                       self.fsm.keepalive_timer.stop();
                       self.fsm.hold_timer.stop();
                        self.fsm.state = State::Idle;
                       println!("Moving to {:#?}", self.fsm.state);
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



    // pub async fn handle_open_message(&mut self, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), BGPError> {
    //     // TODO handle better, for now just accept the neighbor and mirror the capabilities for testing
    //     // compare the open message params to configured neighbor
    //     let received_open = extract_open_message(tsbuf)?;
    //
    //     //let peer_ip = get_neighbor_ipv4_address_from_stream(tcp_stream)?;
    //
    //     if self.is_established() {
    //         return Err(NeighborError::NeighborAlreadyEstablished.into())
    //     }
    //
    //
    //     //let neighbor = bgp_proc.neighbors.get_mut(&peer_ip).ok_or_else(|| NeighborError::PeerIPNotRecognized)?;
    //     let open_message = OpenMessage::new(BGPVersion::V4, bgp_proc.global_settings.my_as, self.fsm.hold_time, bgp_proc.global_settings.identifier, 0, None)?;
    //     send_open(tcp_stream, open_message).await?;
    //     send_keepalive(tcp_stream).await?;
    //     //TODO handle this error so it doesn't return
    //     self.set_hold_time(received_open.hold_time)?;
    //     self.fsm.state = State::Established;
    //
    //     //add_neighbor_from_message(bgp_proc, &mut received_open, peer_ip, cn.hello_time, cn.hold_time)?;
    //
    //     // TODO REMOVE AFTER TESTING IT WORKS HERE
    //     test_net_advertisements(bgp_proc, tcp_stream).await?;
    //
    //     Ok(())
    //
    // }

    pub async fn handle_update_message(&mut self, tsbuf: &Vec<u8>) -> Result<(), BGPError> {
        println!("Handling update message");
        if !self.is_established() {
            return Err(NeighborError::PeerIPNotEstablished.into())
        }
        let update_message = extract_update_message(tsbuf)?;
        self.process_routes_from_message(update_message).await?;

        Ok(())
    }


    // pub async fn route_incoming_message_to_handler(&mut self, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>, bgp_proc: &mut BGPProcess) -> Result<(), BGPError> {
    //     let message_type = parse_packet_type(&tsbuf)?;
    //     println!("{}", message_type);
    //     match message_type {
    //         MessageType::Open => {
    //             self.handle_open_message(tcp_stream, tsbuf, bgp_proc).await?
    //         },
    //         MessageType::Update => {
    //             self.handle_update_message(tcp_stream, tsbuf, bgp_proc).await?
    //         },
    //         MessageType::Notification => {
    //             crate::messages::notification::handle_notification_message(tcp_stream, tsbuf, bgp_proc).await?;
    //         },
    //         MessageType::Keepalive => {
    //             handle_keepalive_message(tcp_stream).await?
    //         },
    //         MessageType::RouteRefresh => {
    //             // TODO handle route refresh
    //             crate::messages::route_refresh::handle_route_refresh_message(tcp_stream, tsbuf)?
    //         }
    //     }
    //     Ok(())
    // }




    pub fn is_established(&self) -> bool {
        if self.fsm.state == State::Established { true }
        else { false }
    }

    pub async fn check_timers_and_generate_events(&mut self) {

        //println!("executing check_timers_and_generate_events");
        if let Ok(true) = self.fsm.connect_retry_timer.is_elapsed() {
            println!("connect_retry_timer elapsed for neighbor {:#?}, generating event", self.ip);
            self.generate_event(Event::ConnectRetryTimerExpires);
        }
        if let Ok(true) = self.fsm.hold_timer.is_elapsed() {
            println!("hold_timer elapsed for neighbor {:#?}, generating event", self.ip);
            self.generate_event(Event::HoldTimerExpires);
        }
        if let Ok(true) = self.fsm.keepalive_timer.is_elapsed() {
            println!("keepalive_timer elapsed for neighbor {:#?}, generating event", self.ip);
            self.generate_event(Event::KeepaliveTimerExpires);
        }
        if let Ok(true) = self.fsm.delay_open_timer.is_elapsed() {
            println!("delay_open_timer elapsed for neighbor {:#?}, generating event", self.ip);
            self.generate_event(Event::DelayOpenTimerExpires);
        }
        if let Ok(true) = self.fsm.idle_hold_timer.is_elapsed() {
            println!("idle_hold_timer elapsed for neighbor {:#?}, generating event", self.ip);
            self.generate_event(Event::IdleHoldTimerExpires);
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
                println!("Generating Event::OpenMsg for neighbor {:#?}", self.ip);
                self.generate_event(Event::OpenMsg(received_msg));
            },
            MessageType::Update => {
                let received_msg = extract_update_message(tsbuf)?;
                println!("Generating Event::UpdateMsg for neighbor {:#?}", self.ip);
                self.generate_event(Event::UpdateMsg(received_msg));
            },
            MessageType::Notification => {
                // TODO create the func
                // let received_msg = extract_notification_message(tsbuf)?;
                println!("Generating Event::NotifMsg for neighbor {:#?}", self.ip);
                //self.generate_event(Event::NotifMsg(received_msg));
            },
            MessageType::Keepalive => {
                println!("Generating Event::KeepAliveMsg for neighbor {:#?}", self.ip);
                self.generate_event(Event::KeepAliveMsg);
            },
            MessageType::RouteRefresh => {
                // TODO create the func
                //let received_msg = extract_route_refresh_message(tsbuf)?;
                println!("Generating Event::RouteRefreshMsg for neighbor {:#?}", self.ip);
                //self.generate_event(Event::RouteRefreshMsg(received_msg));
            }
        }
        Ok(())
    }
}

pub async fn reestablish_neighbor_streams(neighbor_arc: &Arc<Mutex<Neighbor>>) -> Option<OwnedReadHalf> {
    let mut neighbor = neighbor_arc.lock().await;
    //if neighbor.channel.is_active && !neighbor.is_established() {
    if !neighbor.is_established() {
        if let Some(new_tcp_stream) = neighbor.channel.recv_tcp_conn_from_bgp_proc() {
            let (tcp_r_stream, mut tcp_wr_stream) = new_tcp_stream.into_split();
            neighbor.tcp_write_stream = Some(tcp_wr_stream);
           return Some(tcp_r_stream)
        }
    }
    None
}

pub async fn run_neighbor_loop(mut tcp_stream: TcpStream, mut neighbor: Neighbor, peer_ip: Ipv4Addr ) -> Result<(), BGPError> {
    //pub async fn run(&mut self, tcp_stream: TcpStream) {

    // max bgp msg size should never exceed 4096
    // TODO determine if we want to honor the above rule or not, if not, how should we handle it
    let mut tsbuf: Vec<u8> = Vec::with_capacity(65536);
    //let tcp_stream = ts;


    let (mut tcp_read_stream, mut tcp_write_stream) = tcp_stream.into_split();

    // we only store the write half of the stream in the neighbor, not the read half. This is done to prevent locking the neighbor.
    neighbor.tcp_write_stream = Some(tcp_write_stream);
    let neighbor_arc = Arc::new(Mutex::new(neighbor));
    // generates time events
    run_timer_loop(Arc::clone(&neighbor_arc), peer_ip).await;
    // poops and handles events
    run_event_loop(Arc::clone(&neighbor_arc)).await;


    loop {
        // the write half is moved inside the func, whereas the read half is returned here and move it into the correct var
        if let Some(new_tcp_read_stream) = reestablish_neighbor_streams(&neighbor_arc).await {
            tcp_read_stream = new_tcp_read_stream;
            let mut neighbor = neighbor_arc.lock().await;
            neighbor.generate_event(Event::TcpCRAcked);
        }


        tsbuf.clear();
        // TODO move the TCP stream to its own task, and also create a message queue handler similar to our event handler
        // The above will allow us to not block the event handler
        // I could also use message passing
        // Also will look into Arc Mutex for the TCP stream as that would be really simple.
        // I don't want to go Arc Mutex crazy because we'll just end up locking a lot.




        tcp_read_stream.readable().await.unwrap();

        match tcp_read_stream.try_read_buf(&mut tsbuf) {
            Ok(0) => {
                {
                    let mut neighbor = neighbor_arc.lock().await;

                    if neighbor.is_established() {
                        neighbor.generate_event(Event::TcpConnectionFails);
                    }
                    // if neighbor.fsm.state != State::Idle && !neighbor.fsm.passive_tcp_establishment {
                    //     neighbor.generate_event(Event::TcpConnectionFails);
                    // }
                    else if neighbor.fsm.state == State::Idle && neighbor.fsm.passive_tcp_establishment {
                        neighbor.generate_event(Event::AutomaticStartWithPassiveTcpEstablishment);
                    }


                    //println!("matched Ok(0) inside of tcp_read_stream.try_read_buf(&mut tsbuf), generating Event::TcpConnectionFails");
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
                                let mut neighbor = neighbor_arc.lock().await;
                                if let Err(e) =  parse_packet_type(m).and_then(|mt| neighbor.generate_event_from_message(&tsbuf, mt)) {
                                    println!("Error: {:#?}, skipping message", e);
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
                    //return Err(NeighborError::TCPConnDied.into());
                    {
                        let mut neighbor = neighbor_arc.lock().await;
                        if neighbor.is_established() {
                            println!("Error : Unable to use TCP Stream -  {:#?}, generating Event::TcpConnectionFails", e);
                            neighbor.generate_event(Event::TcpConnectionFails);
                        }
                        else if neighbor.fsm.state == State::Idle && neighbor.fsm.passive_tcp_establishment {
                            neighbor.generate_event(Event::AutomaticStartWithPassiveTcpEstablishment);
                        }


                    }
                }
            }
            //let size = ts.read(&mut tsbuf[..]).unwrap();
            //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
        }

    }
}
