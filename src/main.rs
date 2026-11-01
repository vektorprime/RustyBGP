use std::{io::{Read, Write}};
use std::net::Ipv4Addr;

use crate::messages::keepalive::*;
use crate::messages::open::*;
use crate::process::*;

use tokio::sync::{Mutex, mpsc};
use std::sync::Arc;

use anyhow::Result;

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

mod tui;

#[tokio::main]
async fn main()  {

    let (log_tx, log_rx) = mpsc::channel::<String>(255);
    tui::init_tui(log_rx);
    //let mut bgp = BGPProcess::new("bgp_config.toml".to_string());
    let mut bgp: Arc<Mutex<BGPProcess>> = Arc::new(Mutex::new(BGPProcess::new("bgp_config.toml", log_tx)));
    println!("{:#?}", bgp);
    // todo read the IP and port from config file
    BGPProcess::run_process_loop(bgp, "10.0.0.3", "179").await;

}



