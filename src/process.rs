
use std::net::{IpAddr, Ipv4Addr};
use std::error::Error;
use std::io;
use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::{TcpListener};
use tokio::sync::Mutex;

use crate::messages::*;
use crate::neighbors::*;
use crate::config::*;
use crate::errors::{BGPError, NeighborError};
use crate::messages::open::get_neighbor_ipv4_address_from_stream;
use crate::utils::*;
use crate::finite_state_machine::*;
use crate::messages::update::AS::AS4;

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

#[derive(Debug)]
pub struct BGPProcess {
    pub(crate) my_as: u16,
    pub(crate) identifier: Ipv4Addr,
    //pub active_neighbors: Vec<Neighbor>,
    pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub configured_neighbors: Vec<NeighborConfig>,
    pub configured_networks: Vec<NetAdvertisementsConfig>,
}


impl BGPProcess {
    pub fn new(config_file_name: String) -> Self {
        let config = read_config_file(config_file_name);
        BGPProcess {
            my_as: config.process_config.my_as,
            identifier: Ipv4Addr::from_str(&config.process_config.router_id).unwrap(),
            //active_neighbors: Vec::new(),
            neighbors: HashMap::new(),
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

    pub fn is_neighbor_established(&self, peer_ip: Ipv4Addr) -> bool {
        if let Some(n) =  self.neighbors.get(&peer_ip) {
            if n.fsm.state == State::Established {
                return true
            }
        }
        false
    }


    // pub fn is_neighbor_established(&self, peer_ip: Ipv4Addr) -> bool {
    //     if let Some(_) =  self.neighbors.get(&peer_ip) {
    //         return true
    //     }
    //     false
    // }

    pub fn remove_established_neighbor(&mut self, ip: Ipv4Addr) -> Result<(), NeighborError> {
        match self.neighbors.remove(&ip) {
            Some(_) => {
                println!("Removed neighbor : {}", ip.to_string());
                Ok(())
            },
            None => {
                println!("ERROR: {:#?}", NeighborError::UnableToRemoveNeighbor);
                Err(NeighborError::UnableToRemoveNeighbor)
            }
        }
    }

    pub fn validate_neighbor_ip_is_configured(&mut self, peer_ip: IpAddr) -> Result<(), NeighborError> {
        if self.neighbors.is_empty() {
            return Err(NeighborError::ConfiguredNeighborsEmpty)
        }
        for n in &self.neighbors {
            let ip = get_neighbor_ipv4_address(peer_ip)?;
            // TODO refactor string comparison out as it's inefficient
            if *n.0 == peer_ip {
                println!("Validated neighbor IP is in configured neighbors");
                return Ok(())
            }
            else {
                return Err(NeighborError::PeerIPNotRecognized)
            }


        }

        Err(NeighborError::ConfiguredNeighborsEmpty)

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

    pub fn populate_neighbors_from_config(&mut self) {
        for nc in &self.configured_neighbors {
            match Ipv4Addr::from_str(&nc.ip) {
                Ok(ip) => {
                    let peer_type = if self.my_as == nc.as_num {
                        PeerType::Internal
                    } else {
                        PeerType::External
                    };
                    match Neighbor::new(ip, AS4(nc.as_num as u32), nc.hello_time, nc.hold_time, peer_type) {
                        Ok(neighbor) => {
                            self.neighbors.insert(ip, neighbor);
                        },
                        Err(e) => {
                            println!("Unable to create neighbor (IP) {:#?} from Config, ERROR: {:#?}", ip, e);
                        }
                    }
                },
                Err(_) => {
                    println!("ERROR: Unable to convert config string to IP, skipping");
                    continue;
                }
            }


        }
    }

    pub async fn run(bgp_proc: Arc<Mutex<BGPProcess>>, address: String, port: String) {
        //let mut connection_buffers: Vec<Vec<u8>> = Vec::new();
        {
            let mut bgp = bgp_proc.lock().await;
            bgp.populate_neighbors_from_config();
        }
        let listener = start_tcp(address, port).await;
        loop {
            match listener.accept().await {
                Ok((mut tcp_stream, sa)) => {
                    let bgp_arc = Arc::clone(&bgp_proc);
                    println!("TCP connection established from {}", sa.ip().to_string());
                    let peer_ip = tcp_stream.peer_addr().unwrap().ip();
                    {
                        let mut bgp = bgp_arc.lock().await;
                        if let Err(e) = bgp.validate_neighbor_ip_is_configured(peer_ip) {
                            println!("Error validating neighbor IP: {:#?}, skipping", e);
                            continue;
                        }
                        let ip = get_neighbor_ipv4_address(peer_ip);
                         match ip {
                             Ok(ip) => {
                                 let neighbor = bgp.neighbors.get_mut(&ip);
                                 match neighbor {
                                     Some(n) => {
                                         let bgp_arc = Arc::clone(&bgp_proc);
                                         //tokio::spawn(async move {
                                             // get the neighbor and pass the tcp conn
                                             n.run(tcp_stream, bgp_arc).await;
                                         //});
                                     },
                                     None => {
                                         println!("ERROR: Unable to get a handle to the neighbor in bgp.run()");
                                     }
                                 }
                             },
                             Err(e) => println!("{:#?}",e )
                         }
                    }

                    },
                Err(e) => {
                    println!("Error : {:#?}", e);
                }
            }
        }
    }
}