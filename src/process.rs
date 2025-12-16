
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
use crate::errors::{NeighborError};
use crate::messages::open::get_neighbor_ipv4_address_from_stream;
use crate::utils::*;

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
    pub established_neighbors: HashMap<Ipv4Addr, Neighbor>,
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
            established_neighbors: HashMap::new(),
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
        if let Some(_) =  self.established_neighbors.get(&peer_ip) {
            return true
        }
        false
    }

    pub fn remove_established_neighbor(&mut self, ip: Ipv4Addr) -> Result<(), NeighborError> {
        match self.established_neighbors.remove(&ip) {
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

    pub fn validate_neighbor_ip(&mut self, peer_ip: IpAddr) -> Result<(), NeighborError> {
        // we don't have enough info to add the neighbor yet, so we just validate the IP for now

        if self.configured_neighbors.is_empty() {
            return Err(NeighborError::ConfiguredNeighborsEmpty)
        }
        for cn in &self.configured_neighbors {
            println!("Found configured neighbor");
            let ip = get_neighbor_ipv4_address(peer_ip)?;
            if ip.to_string() == cn.ip {
                return Ok(())
            }
            else {
                return Err(NeighborError::PeerIPNotRecognized)
            }


        }

       Err(NeighborError::ConfiguredNeighborsEmpty)

    }

    pub fn handle_tcp_event(&mut self) {
        // maybe at some point I will react to TCP events, or I'll rely on errors from write or read
    }

    pub async fn run(bgp_proc: Arc<Mutex<BGPProcess>>, address: String, port: String) {
        //let mut connection_buffers: Vec<Vec<u8>> = Vec::new();
        let listener = start_tcp(address, port).await;
        loop {
            match listener.accept().await {
                Ok((mut ts, sa)) => {
                    let bgp_proc = Arc::clone(&bgp_proc);
                    println!("TCP connection established from {}", sa.ip().to_string());
                    let peer_ip = ts.peer_addr().unwrap().ip();
                    {
                        let mut bgp = bgp_proc.lock().await;
                        if let Err(e) = bgp.validate_neighbor_ip(peer_ip) {
                            println!("Error validating neighbor IP: {:#?}", e);
                        }
                        println!("Validated neighbor IP");
                    }


                    //check if I've sent all active neighbors updates

                    tokio::spawn(async move {
                        // max bgp msg size should never exceed 4096
                        // TODO determine if we want to honor the above rule or not, if not, how should we handle it
                        let mut tsbuf: Vec<u8> = Vec::with_capacity(65536);
                        loop {
                            ts.readable().await.unwrap();
                            //read tcp stream into buf and save size read
                            let ip = match get_neighbor_ipv4_address_from_stream(&ts) {
                                Ok(i) => i,
                                Err(_) => {continue}
                            };
                            match ts.try_read_buf(&mut tsbuf) {
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
                                                    match route_incomming_message_to_handler(&mut ts, m, &mut *bgp).await {
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
                },
                Err(e) => {
                    println!("Error : {:#?}", e);
                }
            }
        }
    }
}