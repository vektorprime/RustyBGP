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



fn convert_message_to_bytes(message_header: MessageHeader) -> Vec<u8> {
    let mut message: Vec<u8> = Vec::new();


    for b in message_header.marker {
        message.push(b);
    }

    match message_header.message_type {
            // prepare fields, calculate the size, then add to vec
            // TODO handle this better
        MessageType::Open => {
            let mut len: u16 = message.len() as u16;
            len += 2;

            let msg_type: u8 = 1;
            len += 1;

            let len_bytes: [u8; 2] = len.to_le_bytes();
            let len_lo = len_bytes[0];
            let len_hi = len_bytes[1];
    
            message.push(len_lo); // len
            message.push(len_hi); // len p2

            message.push(msg_type); // type

        },
        MessageType::Update => {
            message.push(2);
        },
        MessageType::Notification => {
            message.push(3);
        },
        MessageType::Keepalive => {

            let mut len: u16 = message.len() as u16;
            len += 2;

            let msg_type: u8 = 4;
            len += 1;

            let len_bytes: [u8; 2] = len.to_le_bytes();
            let len_lo = len_bytes[0];
            let len_hi = len_bytes[1];
    
            message.push(len_lo); // len
            message.push(len_hi); // len p2

            message.push(msg_type); // type

        },
    }


    message

}

fn send_keepalive(stream: &mut TcpStream) {
    let message_header = build_message_header(MessageType::Keepalive);
    let message = convert_message_to_bytes(message_header);
    stream.write_all(&message).unwrap();
}

fn send_open(stream: &mut TcpStream) {
    let message_header = build_message_header(MessageType::Open);
    let message = convert_message_to_bytes(message_header);
    stream.write_all(&message).unwrap();
}

fn main() {
    let mut connection_buffers: Vec<Vec<u8>> = Vec::new();

    let listener = start_tcp("170");
    for stream in listener.incoming() {
        let stream = stream;
        match stream {
            Ok(mut ts) => {
                println!("Connection established from {}", ts.peer_addr().unwrap());
                // TODO test printing the stream data
                let mut tsbuf = vec![0; 64];
                let size = ts.read(&mut tsbuf).unwrap();
                println!("Read {} bytes from the stream. ", size);
                let hex = tsbuf[..size]
                    .iter()
                    .map(|b| format!("{:02X} ", b))
                    .collect::<String>();
                println!("Data read from the stream: {}", hex);
                //println!("Data read from the stream: {:#x?}", &tsbuf.get(..size).unwrap());
                let message_type = parse_packet_type(&tsbuf[..size]);
                println!("Message type is: {:?}", message_type);

                let message_header = build_message_header(MessageType::Open);
                let open_message = OpenMessage {
                    message_header,
                    version: BGPVersion::V4,
                    as_number: 1,
                    hold_time: 60,
                    identifier: Ipv4Addr::new(1, 1, 1, 1),
                    optional_parameters_length: 0,
                    optional_parameters: None
                };
                send_open(&mut ts);
                send_keepalive(&mut ts);
                println!("Sent keepalive");
               // connection_buffers.push(tsbuf);

            },
            Err(e) => {
                eprintln!("Error in stream : {}", e);
            }
        }
    }
}



