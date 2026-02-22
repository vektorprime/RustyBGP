use std::net::IpAddr;
use std::fs;
use std::ops::Mul;
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
    pub default_local_preference: u32,
    pub default_med: u32,
    pub capabilities_config: CapabilitiesConfig
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
pub struct CapabilitiesConfig {
    pub route_refresh_prestandard: bool,
    pub route_refresh: bool,
    pub enhanced_route_refresh: bool,
    pub extended_4byte_asn: bool,
    pub multi_protocol_extensions_config: MultiProtocolExtensionsConfig
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy, Default)]
pub struct MultiProtocolExtensionsConfig {
    pub ipv4_unicast: bool,
    pub ipv4_multicast: bool,
    pub ipv4_vpn: bool,
    pub ipv6_unicast: bool,
    pub ipv6_multicast: bool,
    pub ipv6_vpn: bool,
}



impl MultiProtocolExtensionsConfig {
    // pub fn convert_to_bytes(&self) -> Vec<u8> {
    //     let mut bytes = Vec::new();
    //
    // }
    pub fn new(afi: u16, safi: u8) -> Self {

        let mut mp_ex_config = MultiProtocolExtensionsConfig::default();
        match (afi, safi) {
            (1, 1) => {
                mp_ex_config.ipv4_unicast = true;
            },
            (1, 2) => {
                mp_ex_config.ipv4_multicast = true;
            },
            (1, 128) => {
                mp_ex_config.ipv4_vpn = true;
            },
            (2, 1) => {
                mp_ex_config.ipv6_unicast = true;
            },
            (2, 2) => {
                mp_ex_config.ipv6_multicast = true;
            },
            (2, 128) => {
                mp_ex_config.ipv6_vpn = true;
            },
            (_, _) => {
                println!("MultiProtocol afi and safi not recognized, defaulting");
            }


        }

        mp_ex_config
    }
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