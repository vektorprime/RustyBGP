
use std::net::{IpAddr, Ipv4Addr};

use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener};
use tokio::sync::Mutex;


use crate::config::*;
use crate::errors::*;
use crate::finite_state_machine::events::Event;
use crate::messages::BGPVersion;
use crate::utils::*;
use crate::messages::update::AS::AS4;
use crate::neighbors;
use crate::neighbors::{Neighbor, PeerType};

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
pub struct BGPProcess {
    pub global_settings: GlobalSettings,
    //pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub configured_neighbors: Vec<NeighborConfig>,
    pub configured_networks: Vec<NetAdvertisementsConfig>,
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

    pub async fn generate_event_for_all_neighbors(all_neighbors: &Arc<Mutex<HashMap<Ipv4Addr, Neighbor>>>, event: Event) {
        let mut neighbors = all_neighbors.lock().await;

        for n in &mut *neighbors {
            n.1.events.push_back(event.clone());
        }
    }

    pub async fn run(bgp_proc: Arc<Mutex<BGPProcess>>, address: String, port: String) {
        // init
        let all_neighbors =BGPProcess::populate_neighbors_from_config(&bgp_proc).await;
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
                        if let Err(e) = neighbors::run(tcp_stream, bgp_arc, all_neighbors_arc, peer_ip).await {
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

    pub async fn populate_neighbors_from_config(bgp_proc_arc: &Arc<Mutex<BGPProcess>>,) -> Arc<Mutex<HashMap<Ipv4Addr, Neighbor>>> {
        let bgp_proc = bgp_proc_arc.lock().await;
        let mut all_neighbors = HashMap::new();
        for nc in &bgp_proc.configured_neighbors {
            match Ipv4Addr::from_str(&nc.ip) {
                Ok(ip) => {
                    let peer_type = if bgp_proc.global_settings.my_as == nc.as_num {
                        PeerType::Internal
                    } else {
                        PeerType::External
                    };
                    match Neighbor::new(ip, AS4(nc.as_num as u32), nc.hello_time, nc.hold_time, peer_type, &bgp_proc.global_settings) {
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

        Arc::new(Mutex::new(all_neighbors))
    }
}