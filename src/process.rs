
use crate::messages::update::MultiExitDisc;
use std::net::{IpAddr, Ipv4Addr};

use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, broadcast};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::config::*;
use crate::errors::*;
use crate::finite_state_machine::events::Event;
use crate::messages::BGPVersion;
use crate::utils::*;
use crate::messages::update::AS::AS4;
use crate::{neighbors, process};
use crate::channels::{ChannelWatcherMessage, ChannelMessage, NeighborChannel};
use crate::messages::update::{AsPath, AsPathSegment, AsPathSegmentType, LocalPref, NextHop, Origin, OriginType, AS};
use crate::neighbors::{Neighbor, PeerType};
use crate::routes::{RouteV4, NLRI};
use crate::messages::optional_parameters::*;

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

#[derive(Debug, Clone)]
pub struct GlobalSettings {
    pub my_as: u16,
    pub identifier: Ipv4Addr,
    pub next_hop_ip: Ipv4Addr,
    pub version: BGPVersion,
    pub default_local_preference: u32,
    pub default_med: u32,
    pub optional_parameters: OptionalParameters
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum BestPathResult {
    CandidatePath,
    CurrentPath,
    Tie,
}

#[derive(Debug)]
pub struct BGPProcess {
    pub global_settings: GlobalSettings,
    //pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub configured_neighbors: Vec<NeighborConfig>,
    pub configured_networks: Vec<NetAdvertisementsConfig>,
    // TODO changes to loc-rib generate events to all neighbors to send update
    pub adj_rib_in: HashMap<NLRI, Vec<RouteV4>>,
    pub local_rib: HashMap<NLRI, Vec<RouteV4>>,
    //pub neighbors_channels: HashMap<Ipv4Addr, NeighborChannel>, // moved to it's own var so we can lock it separately from the bgp proc
}

impl BGPProcess {

    pub fn new(config_file_name: &str) -> Self {
        let config = read_config_file(config_file_name);

        let global_settings = GlobalSettings {
            my_as: config.process_config.my_as,
            identifier: Ipv4Addr::from_str(&config.process_config.router_id).unwrap(),
            next_hop_ip: Ipv4Addr::from_str(&config.process_config.next_hop_ip).unwrap(),
            default_local_preference: config.process_config.default_local_preference,
            default_med: config.process_config.default_med,
            version: BGPVersion::V4,
            optional_parameters: OptionalParameters::new(
                config.process_config.capabilities_config.multi_protocol_extensions_config,
                config.process_config.capabilities_config.route_refresh_prestandard,
                config.process_config.capabilities_config.route_refresh,
                config.process_config.capabilities_config.enhanced_route_refresh,
                config.process_config.capabilities_config.extended_4byte_asn,
                Some(config.process_config.my_as as u32)
            )
        };

        BGPProcess {
            global_settings,
            //neighbors: HashMap::new(),
            configured_neighbors: config.neighbors_config,
            configured_networks: config.net_advertisements_config,
            adj_rib_in: HashMap::new(),
            local_rib: HashMap::new(),
            //neighbors_channels: HashMap::new(),
        }
    }

    pub fn calc_best_path() {
        // TODO signal new routes in adj rib in so we can trigger this func, maybe add a delay so we receive all routes before running this func
        // since our router will be control-plane only, there will be some differences in between us, Cisco, and the RFC implementations
        // e.g. idc about next-hop being reachable

        // we need to process bgp adj rib in and store in bgp local rib
        
        
        
        // if ebgp
        // check if our AS is in the path


        // run best path:
        //     invoke when we receive an update with a new, replacement, or withdrawn route
        // find all routes to the destination to compare them

        // check for highest weight, default is 0 (weight is locally significant) - need to introduce

        // prefer highest local pref
        // if learned from ibgp peer
        // use that local pref
        // if learned from ebgp peer
        // use the default local pref configured

        // prefer route that this router originated (order network or redist > aggregate)


        // prefer shortest AS path
        // if AS_SET present that counts as 1
        // if "bgp bestpath as-path ignore" this step is skipped - cisco specific
        // if confed as set consider as 0

        // prefer lowest origin number ( order IGP, EGP, incomplete where IGP is network or aggregate commands, egp deprecated, incomplete redisted)


        // if routes are from same neighbor AS, then prefer lowest MED, missing MED means 0, ignore confed sub as
        // if ibgp peer sent you this route
        // if they didn't originate it
        // consider the external AS in the AS path for comparing MED.
        // if they originated it or aggregated it
        // then use the local AS for comparing MED

        // prefer ebgp over ibgp

        // prefer lowest BGP RID

        // prefer lowest BGP peer IP


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
            println!("populating local rib from config");
            println!("configured_network is {:#?}", configured_network);
            let nlri = configured_network.nlri.clone();
            let origin = Origin::new(OriginType::IGP);
            let as_path = self.generate_local_as_path_for_advertisement();
            let next_hop = NextHop::new(self.global_settings.next_hop_ip);
            let local_pref = Some(LocalPref::new(self.global_settings.default_local_preference));
            let med = Some(MultiExitDisc::new(self.global_settings.default_med));
            let atomic_aggregate = None;
            let aggregator = None;
            let new_route = RouteV4::new(nlri.clone(), origin, as_path, next_hop , local_pref, med, atomic_aggregate, aggregator);

            self.adj_rib_in.insert(nlri, vec![new_route]);
        }
    }

     async fn populate_local_rib_from_config_arc(bgp_proc_arc: &Arc<Mutex<BGPProcess>>) {
         let mut bgp_proc = bgp_proc_arc.lock().await;
         bgp_proc.populate_local_rib_from_config();
     }


    pub fn init_process_channels() -> Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>> {
        let all_neighbors_channels: HashMap<Ipv4Addr, NeighborChannel> = HashMap::new();
        Arc::new(Mutex::new(all_neighbors_channels))
    }

    pub async fn run_process_loop(bgp_proc: Arc<Mutex<BGPProcess>>, address: &str, port: &str) {
        let bgp_proc_arc = Arc::clone(&bgp_proc);
        // init
        BGPProcess::populate_local_rib_from_config_arc(&bgp_proc_arc).await;
        let all_neighbors_channels_arc = BGPProcess::init_process_channels();
        let (tx_channel_watcher, rx_channel_watcher) = mpsc::channel::<ChannelWatcherMessage>(1);
        //let (tx_all_event_channel_watcher, rx_all_event_channel_watcher) = broadcast::channel::<ChannelWatcherMessage>(1);
        let mut all_neighbors = BGPProcess::populate_neighbors_from_config(&bgp_proc, &all_neighbors_channels_arc, tx_channel_watcher).await;
        BGPProcess::run_recv_message_channel_loop(Arc::clone(&bgp_proc), Arc::clone(&all_neighbors_channels_arc), rx_channel_watcher).await;
        BGPProcess::generate_event_for_all_neighbors(&mut all_neighbors, Event::AutomaticStartWithPassiveTcpEstablishment).await;
        //


        let listener = start_tcp(address.to_string(), port.to_string()).await;
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
                                    match neighbor_channel.send_tcp_conn_to_neighbor(tcp_stream) {
                                        Ok(_) => {println!("Sent TCP connection in neighbor channel")}
                                        Err(e) => {println!("ERROR: Unable to send TCP connection in channel - {:#?}", e)}
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    println!("Extracted neighbor from hashmap");
                    let (tx_event_channel_watcher, rx_event_channel_watcher) = mpsc::channel::<ChannelWatcherMessage>(5);
                    neighbor.tx_event_channel_watcher = Some(tx_event_channel_watcher);
                    neighbor.generate_event(Event::TcpCRAcked);
                    println!("Generated Event::TcpCRAcked");
                    tokio::spawn(async move {
                        println!("Moving neighbor to async task and executing run_neighbor_loop");
                        // get the neighbor and pass the tcp conn
                        if let Err(e) = neighbors::run_neighbor_loop(tcp_stream, neighbor, peer_ip, rx_event_channel_watcher).await {
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

    pub async fn populate_neighbors_from_config(bgp_proc_arc: &Arc<Mutex<BGPProcess>>, all_neighbors_channels_arc: &Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>>,
                                                tx_channel_watcher: Sender<ChannelWatcherMessage>) -> HashMap<Ipv4Addr, Neighbor> {
        // This function is dual purpose, return all_neighbors (who we added tx and rx channels to) + add channels to all_neighbors_channels_arc (for us to tx and rx messages from neighbor)
        let bgp_proc = bgp_proc_arc.lock().await;
        let mut all_neighbors = HashMap::new();
        all_neighbors.reserve(2); // 2 seems sensible, a good compromise between size and efficiency
        let my_as = bgp_proc.global_settings.my_as.clone();
        let global_settings = bgp_proc.global_settings.clone();
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

                    match Neighbor::new(ip, AS::AS4(nc.as_num as u32), nc.hello_time, nc.hold_time, peer_type, global_settings.clone(), bgp_channel, tx_channel_watcher.clone()) {
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

    fn compare_route_local_pref(curr_best_path: &RouteV4, candidate_best_path: &RouteV4, def_local_pref: u32) -> BestPathResult {
        // prefer higher local pref
        let candidate_path_local_pref = if candidate_best_path.local_pref.is_some() {
            candidate_best_path.local_pref.as_ref().unwrap().value }
        else {
            def_local_pref
        };

        let curr_best_path_local_pref = if curr_best_path.local_pref.is_some() {
            curr_best_path.local_pref.unwrap().value }
        else {
            def_local_pref
        };

        if candidate_path_local_pref == curr_best_path_local_pref {
            return BestPathResult::Tie
        }

        if candidate_path_local_pref > curr_best_path_local_pref {
            return BestPathResult::CandidatePath
        }

        BestPathResult::CurrentPath
    }

    fn compare_route_as_path(curr_best_path: &RouteV4, candidate_best_path: &RouteV4) -> BestPathResult {
        // prefer shortest AS PATH
        // AS SET counts as 1
        // confed counts as 0
        // maybe I'll come back and do something for as-path ignore when/if I tackle multi-path
        // TODO come back and handle multiple ASPathSegmentType objects here once I refactor that code.

        // Right now, this assumes only one SegmentType can be present

        let candidate_path_as_path_len = candidate_best_path.as_path.as_path_segment.number_of_as;
        let curr_best_path_as_path_len = curr_best_path.as_path.as_path_segment.number_of_as;

        if candidate_path_as_path_len == curr_best_path_as_path_len {
            return BestPathResult::Tie
        }
        if candidate_path_as_path_len < curr_best_path_as_path_len {
            return BestPathResult::CandidatePath
        }
        
        BestPathResult::CurrentPath
    }

    fn compare_route_origin(curr_best_path: &RouteV4, candidate_best_path: &RouteV4) -> BestPathResult {
        // prefer in order: IGP, EGP, incomplete

        match (candidate_best_path.origin.origin_type, curr_best_path.origin.origin_type) {
            (OriginType::IGP, OriginType::IGP) |
            (OriginType::EGP, OriginType::EGP) |
            (OriginType::Incomplete, OriginType::Incomplete) => BestPathResult::Tie,

            (OriginType::IGP, OriginType::EGP) |
            (OriginType::IGP, OriginType::Incomplete) |
            (OriginType::EGP, OriginType::Incomplete) => BestPathResult::CandidatePath,

            _ => BestPathResult::CurrentPath

        }

        //BestPathResult::CurrentPath
    }

    pub async fn run_recv_message_channel_loop(bgp_proc_arc: Arc<Mutex<BGPProcess>>, mut all_neighbors_channels_arc: Arc<Mutex<HashMap<Ipv4Addr, NeighborChannel>>>, rx_channel_watcher: Receiver<ChannelWatcherMessage>) {
        // TODO need to refactor this so we don't loop to unlock the all_neighbors_channels_arc
        // maybe pass a MessageReady event that we await on
        // when at least one neighbor has a message, resume the task
        tokio::spawn( async move {
            let mut watcher = rx_channel_watcher;
            //let mut path_changed = false;
            let mut routes_need_best_path_calc: Vec<NLRI> = Vec::new();
            loop {
                if !routes_need_best_path_calc.is_empty() {
                    // TODO calculate best path if path changed
                    // go through every nlri and find the best metrics
                    let mut bgp_proc = bgp_proc_arc.lock().await;
                    while let Some(rt) = routes_need_best_path_calc.pop() {
                        if let Some(all_paths_for_rt) = bgp_proc.adj_rib_in.get(&rt) {
                            let mut best_path: Option<RouteV4> = None;
                            let best_path_exists = if best_path.is_none() {false} else {true};
                            for candidate_path in all_paths_for_rt {
                                if !best_path_exists {
                                    best_path = Some(candidate_path.clone());
                                } else {
                                    let curr_best_path = best_path.as_ref().unwrap();
                                    // TODO compare
                                    // the eBGP ASN check was already done in the neighbor side, maybe we should move it here but I have
                                    // no way of knowing which neighbor added the route and I don't feel like refactoring a new Option var right now

                                    // TODO I think I will implement weight as an attribute because it's very useful, just not now
                                    //

                                    match BGPProcess::compare_route_local_pref(curr_best_path, candidate_path, bgp_proc.global_settings.default_local_preference) {
                                        BestPathResult::CandidatePath => {
                                            best_path = Some(candidate_path.clone());
                                            continue;
                                        },
                                        BestPathResult::CurrentPath => {
                                            continue;
                                        },
                                        BestPathResult::Tie => {
                                            // move on to next att.
                                        }
                                    }

                                    // not going to implement prefer locally originated (Cisco) or prefer lowest accumulated IGP route (Juniper) for now

                                    match BGPProcess::compare_route_as_path(curr_best_path, candidate_path) {
                                        BestPathResult::CandidatePath => {
                                            best_path = Some(candidate_path.clone());
                                            continue;
                                        },
                                        BestPathResult::CurrentPath => {
                                            continue;
                                        },
                                        BestPathResult::Tie => {
                                            // move on to next att.
                                        }
                                    }

                                    match BGPProcess::compare_route_origin(curr_best_path, candidate_path) {
                                        BestPathResult::CandidatePath => {
                                            best_path = Some(candidate_path.clone());
                                            continue;
                                        },
                                        BestPathResult::CurrentPath => {
                                            continue;
                                        },
                                        BestPathResult::Tie => {
                                            // move on to next att.
                                        }
                                    }

                                    // TODO need to get the ibgp vs ebgp peer property on the route at some point, but not now
                                    // if routes are from same neighbor AS, then prefer lowest MED, missing MED means 0, ignore confed sub as
                                    // if ibgp peer sent you this route
                                    // if they didn't originate it
                                    // consider the external AS in the AS path for comparing MED
                                    // if they originated it or aggregated it
                                    // then use the local AS for comparing MED


                                }
                            }
                        }
                    }
                }
                if let Some(ChannelWatcherMessage::MessageWaiting) = watcher.recv().await {
                    let mut all_neighbors_channels = all_neighbors_channels_arc.lock().await;
                    // TODO check the channel and unlock the bgp Arc Mutex if we need to modify the loc_rib
                    for (neighbor_ip, route_channel) in &mut *all_neighbors_channels {
                        while let Ok(msg) = route_channel.rx.try_recv() {
                            match msg {
                                ChannelMessage::Route(route) => {
                                    // for now, I will just test adding it to the bgp loc_rib
                                    // if an entry for the NLRI exists, add the route path too it don't overwrite
                                    {
                                        let nlri = route.nlri.clone();
                                        // store route here so we know which to run bestpath for later
                                        routes_need_best_path_calc.push(nlri.clone());
                                        let mut bgp_proc = bgp_proc_arc.lock().await;
                                        match bgp_proc.adj_rib_in.get_mut(&nlri) {
                                            Some(route_paths) => {
                                                route_paths.push(route);
                                            }
                                            None => {
                                                bgp_proc.adj_rib_in.insert(nlri, vec![route]);
                                            }
                                        }
                                        println!("Adding route to BGP ADJ RIB IN");
                                        println!("Current BGP ADJ RIB IN is {:#?}", bgp_proc.adj_rib_in);
                                    }
                                    //path_changed = true;
                                },
                                ChannelMessage::WithdrawRoute(nlri_vec) => {
                                    let mut bgp_proc = bgp_proc_arc.lock().await;
                                    for nlri in nlri_vec {
                                        // store route here so we know which to run bestpath for later
                                        routes_need_best_path_calc.push(nlri.clone());
                                        // continue with withdraw
                                        println!("Removing route from BGP Local RIB");
                                        if let None =  bgp_proc.local_rib.remove(&nlri) {
                                            println!("Attempted to remove {:#?} from the BGP local RIB but was unable to find the route", nlri);
                                        }
                                    }
                                    // TODO trigger sending a withdraw message
                                    println!("Current BGP Local RIB is {:#?}", bgp_proc.local_rib);
                                    //path_changed = true;
                                }
                                // ChannelMessage::NeighborDown => {
                                //     // prevent the BGP proc from using the TX channel until we get a NeighborUp
                                // },
                                ChannelMessage::NeighborUp => {
                                    // Allow the BGP proc to send messages (routes) to the Neighbor task
                                    let mut bgp_proc = bgp_proc_arc.lock().await;
                                    for (_nlri, route_vec) in &bgp_proc.adj_rib_in {
                                        println!("Received ChannelMessage::NeighborUp, sending route_vec - {:#?}", route_vec);
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

            }
        });
    }
}

