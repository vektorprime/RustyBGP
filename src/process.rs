use std::io::Read;
use std::net::{IpAddr, TcpListener};
use crate::messages::*;
use crate::neighbors::*;

fn start_tcp(port: &str) -> TcpListener {
    let port: &str = "179";
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

pub struct BGPProcess {
    neighbors: Vec<Neighbor>,
}


impl BGPProcess {
    pub fn new() -> Self {
        BGPProcess {
            neighbors: Vec::new()
        }
    }

    pub fn run(&mut self) {
        //let mut connection_buffers: Vec<Vec<u8>> = Vec::new();
        let listener = start_tcp("170");
        for stream in listener.incoming() {
            match stream {
                Ok(mut ts) => {
                    println!("TCP connection established from {}", ts.peer_addr().unwrap());
                    let peer_ip = ts.peer_addr().unwrap().ip();
                    let neighbor = Neighbor::new(peer_ip);
                    self.neighbors.push(neighbor);
                    println!("Added neighbor {:#?}", ts.peer_addr().unwrap().ip());
                    //let mut tsbuf: Vec<u8> = Vec::new();
                    //tsbuf.try_reserve(65536).unwrap();
                    let mut tsbuf: Vec<u8> = vec![0;65535];
                    loop {
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
                        match extract_messages_from_rec_data(&tsbuf[..size], size) {
                            Ok(messages) => {
                                for m in &messages {
                                    route_incomming_message_to_handler(&mut ts, m);
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