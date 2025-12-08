use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::str::FromStr;
use std::collections::HashMap;

use crate::messages::*;
use crate::neighbors::*;
use crate::routes::*;
use crate::config::*;
use crate::errors::{NeighborError};

fn start_tcp(port: &str) -> TcpListener {

    let listener = TcpListener::bind("0.0.0.0:".to_owned() + port);
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

    pub fn validate_neighbor_ip(&mut self, peer_ip: IpAddr) -> Result<(), NeighborError> {
        // we don't have enough info to add the neighbor yet, so we just validate the IP for now

        for cn in &self.configured_neighbors {
            if peer_ip.is_ipv4() {
                if peer_ip.to_string() == cn.ip {
                    println!("Found configured neighbor");
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

    pub fn run(&mut self) {
        //let mut connection_buffers: Vec<Vec<u8>> = Vec::new();
        let listener = start_tcp("179");
        for stream in listener.incoming() {
            match stream {
                Ok(mut ts) => {
                    println!("TCP connection established from {}", ts.peer_addr().unwrap());
                    let peer_ip = ts.peer_addr().unwrap().ip();
                    match self.validate_neighbor_ip(peer_ip) {
                        Ok(_) => {},
                        Err(e) => {
                            println!("Error validating neighbor IP: {:#?}", e);
                            continue
                        }
                    }
                    // let neighbor = Neighbor::new(peer_ip);
                    // self.neighbors.push(neighbor);
                    // println!("Added neighbor {:#?}", ts.peer_addr().unwrap().ip());
                    //let mut tsbuf: Vec<u8> = Vec::new();
                    //tsbuf.try_reserve(65536).unwrap();
                    let mut tsbuf: Vec<u8> = vec![0;65535];
                    loop {
                        //check if i've sent all active neighbors updates


                        // TODO handle read unwrap
                        //read tcp stream into buf and save size read
                        let size = ts.read(&mut tsbuf[..]).unwrap();
                        println!("Read {} bytes from the stream. ", size);
                        let hex = tsbuf[..size]
                            .iter()
                            .map(|b| format!("{:02X} ", b))
                            .collect::<String>();
                        println!("Data read from the stream: {}", hex);
                        //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
                        match extract_messages_from_rec_data(&tsbuf[..size]) {
                            Ok(messages) => {
                                for m in &messages {
                                    match route_incomming_message_to_handler(&mut ts, m, self) {
                                        Ok(_) => {},
                                        Err(e) => {
                                            println!("Error handling message: {:#?}", e);
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                println!("Error extracting messages: {:#?}", e);
                                continue
                            }
                        }
                    }

                    // connection_buffers.push(tsbuf);

                },
                Err(e) => {
                    eprintln!("Error in stream : {}", e);
                }
            }
        }
    }
}