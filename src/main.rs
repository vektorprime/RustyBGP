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

#[tokio::main]
async fn main()  {
    //let mut bgp = BGPProcess::new("bgp_config.toml".to_string());
    let mut bgp: Arc<Mutex<BGPProcess>> = Arc::new(Mutex::new(BGPProcess::new("bgp_config.toml".to_string())));
    println!("{:#?}", bgp);
    BGPProcess::run(bgp).await;
}



