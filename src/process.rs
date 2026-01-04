
use std::net::{IpAddr, Ipv4Addr};

use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedReadHalf;
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
pub(crate) use crate::channels::{ChannelMessage, NeighborChannel};
use crate::messages::update::{AsPath, AsPathSegment, AsPathSegmentType, LocalPref, NextHop, Origin, OriginType, AS};
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
    pub identifier: Ipv4Addr,
    pub next_hop_ip: Ipv4Addr,
    pub version: BGPVersion,
    pub default_local_preference: u32
}


#[derive(Debug)]
pub struct BGPProcess {
    pub global_settings: GlobalSettings,
    //pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub configured_neighbors: Vec<NeighborConfig>,
    pub configured_networks: Vec<NetAdvertisementsConfig>,
    // TODO changes to loc-rib generate events to all neighbors to send update
    pub local_rib: HashMap<NLRI, Vec<RouteV4>>,
    //pub neighbors_channels: HashMap<Ipv4Addr, NeighborChannel>, // moved to it's own var so we can lock it separately from the bgp proc
}

impl BGPProcess {

    pub fn new(config_file_name: String) -> Self {
        let config = read_config_file(config_file_name);
        let global_settings = GlobalSettings {
            my_as: config.process_config.my_as,
            identifier: Ipv4Addr::from_str(&config.process_config.router_id).unwrap(),
            next_hop_ip: Ipv4Addr::from_str(&config.process_config.next_hop_ip).unwrap(),
            default_local_preference: config.process_config.default_local_preference,
            version: BGPVersion::V4,
        };

        BGPProcess {
            global_settings,
            //neighbors: HashMap::new(),
            configured_neighbors: config.neighbors_config,
            configured_networks: config.net_advertisements_config,
            local_rib: HashMap::new(),
            //neighbors_channels: HashMap::new(),
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

    // pub fn validate_neighbor_ip_is_configured(peer_ip: &Ipv4Addr, neighbors: &HashMap<Ipv4Addr, Neighbor>) -> Result<(), NeighborError> {
    //     if neighbors.is_empty() {
    //         return Err(NeighborError::ConfiguredNeighborsEmpty)
    //     }
    //     if let Some(n) = neighbors.get(&peer_ip) {
    //         println!("Validated neighbor IP is in configured neighbors");
    //         Ok(())
    //     }
    //     else {
    //         Err(NeighborError::PeerIPNotRecognized)
    //     }
    //
    //
    //
    // }

    pub fn validate_neighbor_ip_is_configured(&self, peer_ip: Ipv4Addr) -> Result<(), NeighborError> {
        // we don't have enough info to add the neighbor yet, so we just validate the IP for now

        if self.configured_neighbors.is_empty() {
            return Err(NeighborError::ConfiguredNeighborsEmpty)
        }
        for cn in &self.configured_neighbors {
            // TODO refactor string comparison out as it's inefficient
            if peer_ip.to_string() == cn.ip {
                 println!("Validated neighbor IP is in configured neighbors");
                return Ok(())
            }
            else {
                return Err(NeighborError::NeighborIPNotRecognized)
            }

        }
       Err(NeighborError::ConfiguredNeighborsEmpty)
    }

    // pub fn handle_tcp_event(&mut self) {
    //     // maybe at some point I will react to TCP events, or I'll rely on errors from write or read
    // }

    pub fn generate_local_as_path_for_advertisement(&self) -> AsPath {

        let as_path_segment = {
            AsPathSegment {
                segment_type: AsPathSegmentType::AsSequence,
                number_of_as: 1,
                as_list: vec![AS::AS4(self.global_settings.my_as as u32)]
            }
        };

        AsPath::new(as_path_segment)
    }

    fn populate_local_rib_from_config(&mut self) {
        for configured_network in &self.configured_networks {
            let nlri = configured_network.nlri.clone();
            let origin = Origin::new(OriginType::IGP);
            let as_path = self.generate_local_as_path_for_advertisement();
            let next_hop = NextHop::new(self.global_settings.next_hop_ip);
            let local_pref = Some(LocalPref::new(self.global_settings.default_local_preference));
            let med = None;
            let atomic_aggregate = None;
            let aggregator = None;
            let new_route = RouteV4::new(nlri.clone(), origin, as_path, next_hop , local_pref, med, atomic_aggregate, aggregator);

            self.local_rib.insert(nlri, vec![new_route]);
        }
    }

     async fn populate_local_rib_from_config_wrapper(bgp_proc_arc: &Arc<Mutex<BGPProcess>>) {
         let mut bgp_proc = bgp_proc_arc.lock().await;
         bgp_proc.populate_local_rib_from_config();
     }


    pub fn init_process_channels() -> Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>> {
        let all_neighbors_channels: HashMap<Ipv4Addr, NeighborChannel> = HashMap::new();
        Arc::new(Mutex::new(all_neighbors_channels))
    }

    pub async fn run_process_loop(bgp_proc: Arc<Mutex<BGPProcess>>, address: String, port: String) {
        let bgp_proc_arc = Arc::clone(&bgp_proc);
        // init
        BGPProcess::populate_local_rib_from_config_wrapper(&bgp_proc_arc).await;
        let all_neighbors_channels_arc = BGPProcess::init_process_channels();
        let mut all_neighbors = BGPProcess::populate_neighbors_from_config(&bgp_proc, &all_neighbors_channels_arc).await;
        BGPProcess::run_recv_message_channel_loop(Arc::clone(&bgp_proc), Arc::clone(&all_neighbors_channels_arc)).await;
        BGPProcess::generate_event_for_all_neighbors(&mut all_neighbors, Event::AutomaticStartWithPassiveTcpEstablishment).await;
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
                        let bgp  = bgp_proc_arc.lock().await;
                        if let Err(e) = bgp.validate_neighbor_ip_is_configured(peer_ip) {
                            println!("Error: Unable to validate neighbor IP: {:#?}, skipping", e);
                            continue;
                        }
                    }

                    //let all_neighbors_arc = Arc::clone(&all_neighbors);
                    let mut neighbor = match all_neighbors.remove(&peer_ip) {
                        Some(n) => n,
                        None => {
                            // TODO split the first connection vs resuming/passing a new connection to neighbor into two different funcs
                            println!("Unable to get neighbor object from all_neighbors, which means the neighbor loop is already running");
                            {
                                let mut all_neighbors_channels = all_neighbors_channels_arc.lock().await;
                                println!("unlocked all_neighbors_channels_arc");
                                if let Some(neighbor_channel) =  all_neighbors_channels.get_mut(&peer_ip) {
                                    println!("Got Some(neighbor_channel)");
                                    if let Err(e) = neighbor_channel.send_tcp_conn_to_neighbor(tcp_stream) {
                                        println!("ERROR: Unable to send TCP connection in channel - {:#?}", e);
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    println!("Extracted neighbor from hashmap");
                    neighbor.generate_event(Event::TcpCRAcked);
                    println!("Generated Event::TcpCRAcked");
                    tokio::spawn(async move {
                        println!("Moving neighbor to async task and executing run_neighbor_loop");
                        // get the neighbor and pass the tcp conn
                        if let Err(e) = neighbors::run_neighbor_loop(tcp_stream, neighbor, peer_ip).await {
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

    pub async fn populate_neighbors_from_config(bgp_proc_arc: &Arc<Mutex<BGPProcess>>, all_neighbors_channels_arc: &Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>>) -> HashMap<Ipv4Addr, Neighbor> {
        // This function is dual purpose, return all_neighbors (who we added tx and rx channels to) + add channels to all_neighbors_channels_arc (for us to tx and rx messages from neighbor)
        let bgp_proc = bgp_proc_arc.lock().await;
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
                    let neighbors_channels = NeighborChannel {
                        rx: rx_from_neighbor,
                        tx: tx_to_neighbor,
                        //is_active: true
                    };
                    let bgp_channel = NeighborChannel {
                        rx: rx_from_bgp,
                        tx: tx_to_bgp,
                        //is_active: true,
                    };
                    // need to use a temp HashMap because we already borrowed bgp_proc as mutable
                    {
                        let mut all_neighbors_channels = all_neighbors_channels_arc.lock().await;
                        all_neighbors_channels.insert(ip, neighbors_channels);
                    }

                    match Neighbor::new(ip, AS::AS4(nc.as_num as u32), nc.hello_time, nc.hold_time, peer_type, global_settings, bgp_channel) {
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

        all_neighbors
    }


    pub async fn generate_event_for_all_neighbors(all_neighbors: &mut HashMap<Ipv4Addr, Neighbor>, event: Event) {
        println!("Generating event {:#?} for all neighbors", event);
        for n in all_neighbors {
            n.1.events.push_back(event.clone());
        }
    }



    pub async fn run_recv_message_channel_loop(bgp_proc_arc: Arc<Mutex<BGPProcess>>, mut all_neighbors_channels_arc: Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>>) {
        tokio::spawn( async move {
            loop {
                let mut all_neighbors_channels = all_neighbors_channels_arc.lock().await;
                // TODO check the channel and unlock the bgp Arc Mutex if we need to modify the loc_rib
                for (neighbor_ip, route_channel) in &mut *all_neighbors_channels {
                    while let Ok(msg) = route_channel.rx.try_recv() {
                        match msg {
                            ChannelMessage::Route(route) => {
                                // TODO handle route here so that we go calc. best path and add it to the loc_rib
                                // for now i will just test adding it to the bgp loc_rib
                                // if an entry for the NLRI exists, add the route path too it don't overwrite
                                {
                                    let nlri = route.nlri.clone();
                                    let mut bgp_proc = bgp_proc_arc.lock().await;
                                    match bgp_proc.local_rib.get_mut(&nlri) {
                                       Some(route_paths) => {
                                           route_paths.push(route);
                                       }
                                       None => {
                                           bgp_proc.local_rib.insert(nlri, vec![route]);
                                       }
                                    }
                                    println!("Adding route to BGP local RIB");
                                    println!("Current BGP local RIB is {:#?}", bgp_proc.local_rib);
                                }
                            },
                            ChannelMessage::WithdrawRoute(nlri_vec) => {
                                let mut bgp_proc = bgp_proc_arc.lock().await;
                                for nlri in nlri_vec {
                                    println!("Removing route from BGP local RIB");
                                    if let None =  bgp_proc.local_rib.remove(&nlri) {
                                        println!("Attempted to remove {:#?} from the BGP local RIB but was unable to find the route", nlri);
                                    }
                                }
                                println!("Current BGP Local RIB is {:#?}", bgp_proc.local_rib);
                            }
                            // ChannelMessage::NeighborDown => {
                            //     // prevent the BGP proc from using the TX channel until we get a NeighborUp
                            // },
                            ChannelMessage::NeighborUp => {
                                // Allow the BGP proc to send messages (routes) to the Neighbor task
                                let mut bgp_proc = bgp_proc_arc.lock().await;
                                for (_nlri, route_vec) in &bgp_proc.local_rib {
                                    route_channel.send_route_vec(route_vec).await;
                                }

                            },
                            ChannelMessage::TcpEstablished(tcp_stream) => {
                                panic!("We should never get a NeighborChannel::TcpEstablished from Neighbor to BGP proc");
                            }

                        }
                    }
                }
            }
        });
    }
}

