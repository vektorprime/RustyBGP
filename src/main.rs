use std::{io::{Read, Write}};
use crate::process::*;
use crate::logger::*;

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
mod logger;

#[tokio::main]
async fn main()  {

    // let (log_tx, log_rx) = mpsc::channel::<String>(255);
    // let logger = BGPLogger { tx: log_tx.clone() };
    // log::set_boxed_logger(Box::new(logger)).unwrap();
    // log::set_max_level(log::LevelFilter::Debug);
    // let mut bgp = BGPProcess::new("bgp_config.toml".to_string());

    let rx_log = BGPLogger::setup_logger();
    let tx_tui = tui::init_tui(rx_log);
    let bgp: Arc<Mutex<BGPProcess>> = Arc::new(Mutex::new(BGPProcess::new("bgp_config.toml")));
    log::info!("{:#?}", bgp);
    // todo read the IP and port from config file
    BGPProcess::run_process_loop(bgp, "10.0.0.3", "179", tx_tui).await;
}



