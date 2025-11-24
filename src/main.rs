use std::{io::{Read, Write}, net::{TcpListener, TcpStream}};
use std::net::Ipv4Addr;

use crate::messages::*;


mod routes;
mod neighbors;
mod messages;
mod utils;

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

fn parse_packet_type(tsbuf: &[u8]) -> MessageType {
    match tsbuf.get(0) {
        Some(val) => {
            match tsbuf.get(18) {
                Some(1) => MessageType::Open,
                Some(2) => MessageType::Update,
                Some(3) => MessageType::Notification,
                Some(4) => MessageType::Keepalive,
                _ => panic!("Unknown MessageType in parse_packet_type")
            }
        }
        None => panic!("Unable to read first byte of buffer in parse_packet_type")
    }
}




fn route_incomming_message_to_handler (message_type: MessageType, tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) {
    match message_type {
        MessageType::Open => {
            handle_open_message(tcp_stream, tsbuf);
        },
        MessageType::Update => {
            // TODO handle_update_message
        },
        MessageType::Notification => {
            // TODO handle_notification_message
            send_keepalive(tcp_stream);
        },
        MessageType::Keepalive => {
            // TODO handle_keepalive_message
        },
    }
}

fn handle_open_message(tcp_stream: &mut TcpStream, tsbuf: &Vec<u8>) {
    //TODO handle better, for now just accept the neighbor and mirror the capabilities for testing
    let open_message = OpenMessage::new(2, 180, Ipv4Addr::new(1,1,1,1), 0, None);
    send_open(tcp_stream, open_message);
    send_keepalive(tcp_stream);
}

fn send_open(stream: &mut TcpStream, message: OpenMessage) {
    println!("Preparing to send Open");
    let message_bytes = message.convert_to_bytes();
    stream.write_all(&message_bytes[..]).unwrap();
    println!("Sent Open");
}

fn handle_keepalive_message(tcp_stream: &mut TcpStream) {
    send_keepalive(tcp_stream);
}

fn send_keepalive(stream: &mut TcpStream) {
    //TODO add peer as input var and match against DB
    //TODO add periodic keepalives
    println!("Preparing to send Keepalive");
    let message = KeepaliveMessage::new();
    let message_bytes = message.convert_to_bytes();
    stream.write_all(&message_bytes[..]).unwrap();
    println!("Sent Keepalive");
}


fn main() {
    let mut connection_buffers: Vec<Vec<u8>> = Vec::new();

    let listener = start_tcp("170");
    for stream in listener.incoming() {
        match stream {
            Ok(mut ts) => {
                println!("TCP connection established from {}", ts.peer_addr().unwrap());
                loop {
                    let mut tsbuf: Vec<u8> = vec![0; 64];
                    let size = ts.read(&mut tsbuf[..]).unwrap();
                    println!("Read {} bytes from the stream. ", size);
                    let hex = tsbuf[..size]
                        .iter()
                        .map(|b| format!("{:02X} ", b))
                        .collect::<String>();
                    println!("Data read from the stream: {}", hex);
                    //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
                    let message_type = parse_packet_type(&tsbuf[..size]);
                    println!("Message type is: {:?}", message_type);
                    route_incomming_message_to_handler(message_type, &mut ts, &tsbuf);
                }

               // connection_buffers.push(tsbuf);

            },
            Err(e) => {
                eprintln!("Error in stream : {}", e);
            }
        }
    }
}



