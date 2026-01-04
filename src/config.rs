use std::net::IpAddr;
use std::fs;

use serde::Deserialize;
use toml;

//use crate::messages::update::AS;
use crate::routes::*;
//used to handle the toml configurations

pub fn read_config_file(file_name: String) -> Config {
    let toml_content = fs::read_to_string(file_name).unwrap();
    let config: Config = toml::from_str(&toml_content).unwrap();
    config

}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub process_config: ProcessConfig,
    pub neighbors_config: Vec<NeighborConfig>,
    pub net_advertisements_config: Vec<NetAdvertisementsConfig>
}

#[derive(Debug, Deserialize)]
pub struct ProcessConfig {
    pub my_as: u16,
    pub router_id: String,
    pub next_hop_ip: String,
    pub default_local_preference: u32
}


#[derive(Debug, Deserialize, Clone)]
pub struct NeighborConfig {
    pub ip: String,
    pub as_num: u16,
    pub hello_time: u16,
    pub hold_time: u16,
}

#[derive(Debug, Deserialize)]
pub struct NetAdvertisementsConfig {
    #[serde(rename = "prefix")]
    pub nlri: NLRI
}