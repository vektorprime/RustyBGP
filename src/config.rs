use std::net::IpAddr;
use crate::messages::update::AS;
use serde::Deserialize;
use std::fs;
use toml;
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
}
#[derive(Debug, Deserialize)]
pub struct ProcessConfig {
    pub my_as: u16,
    pub router_id: String
}


#[derive(Debug, Deserialize)]
pub struct NeighborConfig {
    pub ip: String,
    pub as_num: u16,
    pub hello_time: u16,
    pub hold_time: u16,
}