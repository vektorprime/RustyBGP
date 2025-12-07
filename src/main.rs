use std::{io::{Read, Write}, net::{TcpListener, TcpStream}};
use std::net::Ipv4Addr;

use crate::messages::keepalive::*;
use crate::messages::open::*;
use crate::process::*;

mod routes;
mod neighbors;
mod messages;
mod utils;
mod sessions;
mod finite_state_machine;
mod process;
mod timers;
mod errors;
mod config;

fn main() {
    let mut bgp = BGPProcess::new("bgp_config.toml".to_string());
    println!("{:#?}", bgp);
    bgp.run();
}



