use std::{io::{Read, Write}};
use std::net::Ipv4Addr;

use crate::messages::keepalive::*;
use crate::messages::open::*;
use crate::process::*;

use tokio::sync::{Mutex};
use std::sync::Arc;



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
mod channels;

#[tokio::main]
async fn main()  {
    //let mut bgp = BGPProcess::new("bgp_config.toml".to_string());
    let mut bgp: Arc<Mutex<BGPProcess>> = Arc::new(Mutex::new(BGPProcess::new("bgp_config.toml".to_string())));
    println!("{:#?}", bgp);
    // todo read the IP and port from config file
    BGPProcess::run_process_loop(bgp, "10.0.0.3".to_string(), "179".to_string()).await;

}



