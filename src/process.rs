
use std::net::{IpAddr, Ipv4Addr};

use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use crate::config::*;
use crate::errors::*;
use crate::finite_state_machine::events::Event;
use crate::messages::BGPVersion;
use crate::utils::*;
use crate::messages::update::AS::AS4;
use crate::{neighbors, process};
use crate::neighbors::{Neighbor, PeerType};
use crate::routes::{RouteV4, NLRI};

async fn start_tcp(address: String, port: String) -> TcpListener {
    let listener = TcpListener::bind(address + ":" + &port).await;
    match listener {
        Ok(tcp) => {
            println!("TCP server started on port {} ", port);
            tcp
        },
        Err(e) => {
            panic!("Unable to bind port {}, error is {}", port, e);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlobalSettings {
    pub my_as: u16,
    pub identifier:Ipv4Addr,
    pub version: BGPVersion,
}
#[derive(Debug)]
pub struct RouteChannel {
    tx: mpsc::Sender<ChannelMessage>,
    rx: mpsc::Receiver<ChannelMessage>,
    pub is_active: bool,
}

pub enum ChannelMessage {
    Route(RouteV4),
    NeighborDown,
    NeighborUp,
}

impl RouteChannel {
    pub async fn bring_up(&mut self) -> Result<(), EventError> {
        self.is_active = true;
        self.tx.send(ChannelMessage::NeighborUp).await.map_err(|_| EventError::ChannelDown)?;
        Ok(())
    }
    pub async fn take_down(&mut self) -> Result<(), EventError> {
        self.is_active = false;
        self.tx.send(ChannelMessage::NeighborDown).await.map_err(|_| EventError::ChannelDown)?;
        Ok(())
    }
    pub async fn send_route(&self, route: RouteV4) {
        if self.is_active {
            self.tx.send(ChannelMessage::Route(route)).await.unwrap();
        }
    }
}
#[derive(Debug)]
pub struct BGPProcess {
    pub global_settings: GlobalSettings,
    //pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub configured_neighbors: Vec<NeighborConfig>,
    pub configured_networks: Vec<NetAdvertisementsConfig>,
    // TODO changes to loc-rib generate events to all neighbors to send update
    pub loc_rib: HashMap<NLRI, Vec<RouteV4>>,
    pub neighbors_channels: HashMap<Ipv4Addr, RouteChannel>,
}

impl BGPProcess {
    pub fn new(config_file_name: String) -> Self {
        let config = read_config_file(config_file_name);
        let global_settings = GlobalSettings {
            my_as: config.process_config.my_as,
            identifier: Ipv4Addr::from_str(&config.process_config.router_id).unwrap(),
            version: BGPVersion::V4,
        };
        BGPProcess {
            global_settings,
            //neighbors: HashMap::new(),
            configured_neighbors: config.neighbors_config,
            configured_networks: config.net_advertisements_config,
            loc_rib: HashMap::new(),
            neighbors_channels: HashMap::new(),
        }
    }

    pub fn get_neighbor_config(&self, ipv4addr: Ipv4Addr) -> Result<NeighborConfig, NeighborError> {
        let ip = ipv4addr.to_string();
        for cn in &self.configured_neighbors {
            if ip == cn.ip {
                return Ok(cn.clone());
            }
        }
        Err(NeighborError::ConfiguredNeighborNotFound)
    }




    // pub fn is_neighbor_established(&self, peer_ip: Ipv4Addr) -> bool {
    //     if let Some(n) =  self.neighbors.get(&peer_ip) {
    //         if n.fsm.state == State::Established {
    //             return true
    //         }
    //     }
    //     false
    // }


    // pub fn is_neighbor_established(&self, peer_ip: Ipv4Addr) -> bool {
    //     if let Some(_) =  self.neighbors.get(&peer_ip) {
    //         return true
    //     }
    //     false
    // }

    // pub fn remove_established_neighbor(&mut self, ip: Ipv4Addr) -> Result<(), NeighborError> {
    //     match self.neighbors.remove(&ip) {
    //         Some(_) => {
    //             println!("Removed neighbor : {}", ip.to_string());
    //             Ok(())
    //         },
    //         None => {
    //             println!("ERROR: {:#?}", NeighborError::UnableToRemoveNeighbor);
    //             Err(NeighborError::UnableToRemoveNeighbor)
    //         }
    //     }
    // }

    pub fn validate_neighbor_ip_is_configured(peer_ip: &Ipv4Addr, neighbors: &HashMap<Ipv4Addr, Neighbor>) -> Result<(), NeighborError> {
        if neighbors.is_empty() {
            return Err(NeighborError::ConfiguredNeighborsEmpty)
        }
        if let Some(n) = neighbors.get(&peer_ip) {
            println!("Validated neighbor IP is in configured neighbors");
            Ok(())
        }
        else {
            Err(NeighborError::PeerIPNotRecognized)
        }



    }

    // pub fn validate_neighbor_ip_is_configured(&mut self, peer_ip: IpAddr) -> Result<(), NeighborError> {
    //     // we don't have enough info to add the neighbor yet, so we just validate the IP for now
    //
    //     if self.configured_neighbors.is_empty() {
    //         return Err(NeighborError::ConfiguredNeighborsEmpty)
    //     }
    //     for cn in &self.configured_neighbors {
    //         let ip = get_neighbor_ipv4_address(peer_ip)?;
    //         // TODO refactor string comparison out as it's inefficient
    //         if ip.to_string() == cn.ip {
    //              println!("Validated neighbor IP is in configured neighbors");
    //             return Ok(())
    //         }
    //         else {
    //             return Err(NeighborError::PeerIPNotRecognized)
    //         }
    //
    //
    //     }
    //
    //    Err(NeighborError::ConfiguredNeighborsEmpty)
    //
    // }

    // pub fn handle_tcp_event(&mut self) {
    //     // maybe at some point I will react to TCP events, or I'll rely on errors from write or read
    // }





    pub async fn run_process_loop(bgp_proc: Arc<Mutex<BGPProcess>>, address: String, port: String) {
        // init
        let mut all_neighbors_channels: HashMap<Ipv4Addr, RouteChannel> = HashMap::new();
        all_neighbors_channels.reserve(10);
        let all_neighbors = BGPProcess::populate_neighbors_from_config(&bgp_proc, &mut all_neighbors_channels).await;
        BGPProcess::run_channel_loop(Arc::clone(&bgp_proc), all_neighbors_channels).await;
        BGPProcess::generate_event_for_all_neighbors(&all_neighbors, Event::AutomaticStartWithPassiveTcpEstablishment).await;
        //

        let listener = start_tcp(address, port).await;
        loop {
            // TODO handle config sync between proc and neighbors, maybe use an event based thing or just cycle through the neighbors and update
            // TODO generate events here for for overall process (also do it in neighbor run)
            match listener.accept().await {
                Ok((mut tcp_stream, sa)) => {
                    println!("TCP connection established from {}", sa.ip().to_string());
                    let peer_ip = match get_neighbor_ipv4_address_from_socket(tcp_stream.peer_addr()) {
                        Ok(ip) => ip,
                        Err(e) => {
                            println!("Error: TCP Socket error -  {:#?}, skipping", e);
                            continue;
                        }
                    };
                    {
                        let mut all_n = all_neighbors.lock().await;
                        if let Err(e) = BGPProcess::validate_neighbor_ip_is_configured(&peer_ip, &all_n) {
                            println!("Error: Unable to validate neighbor IP: {:#?}, skipping", e);
                            continue;
                        }
                        // TODO make this a helpder function or make it more concise
                        if let Some(n) = all_n.get_mut(&peer_ip) {
                            n.generate_event(Event::TcpCRAcked);
                        }
                    }

                    let bgp_arc = Arc::clone(&bgp_proc);
                    let all_neighbors_arc = Arc::clone(&all_neighbors);
                    tokio::spawn(async move {
                        // get the neighbor and pass the tcp conn
                        if let Err(e) = neighbors::run_neighbor_loop(tcp_stream, bgp_arc, all_neighbors_arc, peer_ip).await {
                            println!("Error: Unable to continue run() for neighbor {:#?} - {:#?}", peer_ip, e);
                        }
                    });
                },
                Err(e) => {
                    println!("Error: TCP Stream {:#?}", e);
                }
            }
        }
    }

    pub async fn populate_neighbors_from_config(bgp_proc_arc: &Arc<Mutex<BGPProcess>>, all_neighbors_channels: &mut HashMap<Ipv4Addr, RouteChannel>) -> Arc<Mutex<HashMap<Ipv4Addr, Neighbor>>> {
        let mut bgp_proc = bgp_proc_arc.lock().await;
        let mut all_neighbors = HashMap::new();
        all_neighbors.reserve(10);
        let my_as = bgp_proc.global_settings.my_as;
        let global_settings = bgp_proc.global_settings;
        for nc in &bgp_proc.configured_neighbors {
            match Ipv4Addr::from_str(&nc.ip) {
                Ok(ip) => {
                    let peer_type = if my_as == nc.as_num {
                        PeerType::Internal
                    } else {
                        PeerType::External
                    };
                    let (tx_to_bgp, rx_from_neighbor)  = mpsc::channel::<ChannelMessage>(65535);
                    let (tx_to_neighbor, rx_from_bgp) = mpsc::channel::<ChannelMessage>(65535);
                    let neighbors_channels = RouteChannel {
                        rx: rx_from_neighbor,
                        tx: tx_to_neighbor,
                        is_active: false
                    };
                    let bgp_channel = RouteChannel {
                        rx: rx_from_bgp,
                        tx: tx_to_bgp,
                        is_active: false,
                    };
                    // need to use a temp HashMap because we already borrowed bgp_proc as mutable
                    all_neighbors_channels.insert(ip, neighbors_channels);
                    match Neighbor::new(ip, AS4(nc.as_num as u32), nc.hello_time, nc.hold_time, peer_type, global_settings, bgp_channel) {
                        Ok(neighbor) => {
                            all_neighbors.insert(ip, neighbor);
                        },
                        Err(e) => {
                            println!("Error: Unable to create neighbor (IP) {:#?} from Config, ERROR: {:#?}", ip, e);
                        }
                    }
                },
                Err(_) => {
                    println!("Error: Unable to convert config string to IP, skipping");
                    continue;
                }
            }
        }
        println!();
        println!("Populated the following BGP neighbors from config {:#?}", all_neighbors);
        println!();

        Arc::new(Mutex::new(all_neighbors))
    }


    pub async fn generate_event_for_all_neighbors(all_neighbors: &Arc<Mutex<HashMap<Ipv4Addr, Neighbor>>>, event: Event) {
        println!("Generating event {:#?} for all neighbors", event);
        let mut neighbors = all_neighbors.lock().await;
        for n in &mut *neighbors {
            n.1.events.push_back(event.clone());
        }
    }

    pub async fn run_channel_loop(bgp_proc_arc: Arc<Mutex<BGPProcess>>,  mut all_neighbors_channels: HashMap<Ipv4Addr, RouteChannel>) {
        tokio::spawn( async move {
            loop {
                // TODO check the channel and unlock the bgp Arc Mutex if we need to modify the loc_rib
                for (neighbor_ip, route_channel) in &mut all_neighbors_channels {
                    while let Some(msg) = route_channel.rx.recv().await {
                        match msg {
                            ChannelMessage::Route(route) => {
                                // TODO handle route here so that we go calc. best path and add it to the loc_rib
                                // for now i will just test adding it to the bgp loc_rib
                                // if an entry for the NLRI exists, add the route path too it don't overwrite
                                {
                                    let nlri = route.nlri.clone();
                                    let mut bgp_proc = bgp_proc_arc.lock().await;
                                   match bgp_proc.loc_rib.get_mut(&nlri) {
                                       Some(route_paths) => {
                                           route_paths.push(route);
                                       }
                                       None => {
                                           bgp_proc.loc_rib.insert(nlri, vec![route]);
                                       }
                                   }
                                    println!("Current BGP Local RIB is {:#?}", bgp_proc.loc_rib);
                                }
                            },
                            ChannelMessage::NeighborDown => {
                                // TODO prevent the BGP proc from using the TX channel until we get a NeighborUp
                            },
                            ChannelMessage::NeighborUp => {
                                // TODO Allow the BGP proc to send messages (routes) to the Neighbor task
                            }
                        }
                    }
                }
            }
        });
    }
}

