
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

async fn start_tcp(port: &str) -> TcpListener {
    let listener = TcpListener::bind("0.0.0.0:".to_owned() + port).await;
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
    pub active_neighbors: HashMap<Ipv4Addr, Neighbor>,
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
            active_neighbors: HashMap::new(),
            configured_neighbors: config.neighbors_config,
            configured_networks: config.net_advertisements_config,
        }
    }

    pub fn is_neighbor_established(&self, peer_ip: Ipv4Addr) -> bool {
        if let Some(_) =  self.active_neighbors.get(&peer_ip) {
            return false
        }
        true
    }


    pub fn validate_neighbor_ip(&mut self, peer_ip: IpAddr) -> Result<(), NeighborError> {
        // we don't have enough info to add the neighbor yet, so we just validate the IP for now

        if self.configured_neighbors.is_empty() {
            return Err(NeighborError::ConfiguredNeighborsEmpty)
        }
        for cn in &self.configured_neighbors {
            println!("Found configured neighbor");
            if let IpAddr::V4(ip) = peer_ip {
                if peer_ip.to_string() == cn.ip {
                    if self.is_neighbor_established(ip) {
                        return Err(NeighborError::NeighborAlreadyEstablished)
                    }
                    return Ok(())
                }
                else {
                    return Err(NeighborError::PeerIPNotRecognized)
                }
            }
            else {
                return Err(NeighborError::NeighborIsIPV6)
            }
        }

       Err(NeighborError::ConfiguredNeighborsEmpty)

    }

    pub fn handle_tcp_event(&mut self) {

    }

    pub async fn run(bgp_proc: Arc<Mutex<BGPProcess>>) {
        //let mut connection_buffers: Vec<Vec<u8>> = Vec::new();

        let listener = start_tcp("179").await;
        loop {
            match listener.accept().await {
                Ok((mut ts, sa)) => {
                    println!("TCP connection established from {}", sa.ip().to_string());
                    let peer_ip = ts.peer_addr().unwrap().ip();
                    {
                        let mut bgp_ulock = bgp_proc.lock().await;
                        if let Err(e) = bgp_ulock.validate_neighbor_ip(peer_ip) {
                            println!("Error validating neighbor IP: {:#?}", e);
                        }
                        println!("Validated neighbor IP");
                    }

                    let bgp = Arc::clone(&bgp_proc);
                    //check if I've sent all active neighbors updates
                    //ts.readable().await;
                    // loop {
                    //
                    // }
                    tokio::spawn(async move {
                        loop {
                            let mut tsbuf: Vec<u8> = Vec::with_capacity(65536);

                            //read tcp stream into buf and save size read
                            match ts.try_read_buf(&mut tsbuf) {
                                Ok(0) => {},
                                Ok(size) => {

                                    println!("Read {} bytes from the stream. ", size);
                                    let hex = tsbuf[..size]
                                        .iter()
                                        .map(|b| format!("{:02X} ", b))
                                        .collect::<String>();
                                    println!("Data read from the stream: {}", hex);
                                    match extract_messages_from_rec_data(&tsbuf[..size]) {
                                        Ok(messages) => {
                                            for m in &messages {
                                                {
                                                    let mut bgp_ulock = bgp.lock().await;
                                                    match route_incomming_message_to_handler(&mut ts, m, &mut *bgp_ulock).await {
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
                                        println!("Error : {:#?}", e)
                                        //return Err(e.into());
                                    }
                                }
                                //let size = ts.read(&mut tsbuf[..]).unwrap();
                                //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
                            }
                        }


                    });


                },
                Err(e) => { println!("Error : {:#?}", e) }
            }
        }
    }
}