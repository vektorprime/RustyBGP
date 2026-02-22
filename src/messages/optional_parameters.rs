
// #[derive(PartialEq, Debug, Clone)]
// //not really needed since the only type we ever use is Capability
// pub enum ParameterType {
//     Capability
//     // others are unassigned and auth is deprecated
// }



// #[derive(PartialEq, Debug, Clone)]
// pub struct Parameter {
//     parameter_type: ParameterType,
// }

use crate::config::MultiProtocolExtensionsConfig;
use crate::errors::MessageError;
use crate::messages::notification::NotifErrorMsgHdrSubCode;
use crate::messages::update::AS;


#[derive(PartialEq, Debug, Clone, Copy)]
pub enum MPExtVal {
    IPv4Unicast,
    IPv4Multicast,
    IPv4VPN,
    IPv6Unicast,
    IPv6Multicast,
    IPv6VPN,
}

impl MPExtVal {
    pub fn new(afi: u16, safi: u8) -> Result<Self, MessageError> {

        match (afi, safi) {
            (1, 1) => {
                Ok(MPExtVal::IPv4Unicast)
            },
            (1, 2) => {
                Ok(MPExtVal::IPv4Multicast)
            },
            (1, 128) => {
                Ok(MPExtVal::IPv4VPN)
            },
            (2, 1) => {
                Ok(MPExtVal::IPv6Unicast)
            },
            (2, 2) => {
                Ok(MPExtVal::IPv6Multicast)
            },
            (2, 128) => {
                Ok(MPExtVal::IPv6VPN)
            },
            (_, _) => {
                println!("MultiProtocol afi and safi not recognized, defaulting");
                Err(MessageError::BadMultiProtocolExtValue)
            }

        }


    }
}

impl TryFrom<&MultiProtocolExtensionsConfig> for MPExtVal {
    type Error = MessageError;

    fn try_from(value: &MultiProtocolExtensionsConfig) -> Result<Self, Self::Error> {
       if value.ipv4_unicast {
           return Ok(MPExtVal::IPv4Unicast)
        } else if value.ipv4_multicast {
           return Ok(MPExtVal::IPv4Multicast)
        } else if value.ipv4_vpn {
           return Ok(MPExtVal::IPv4VPN)
        } else if value.ipv6_unicast {
           return Ok(MPExtVal::IPv6Unicast)
       } else if value.ipv6_multicast {
           return Ok(MPExtVal::IPv6Multicast)
       } else if value.ipv6_vpn {
           return Ok(MPExtVal::IPv6VPN)
       }
        Err(MessageError::NoMPExtValAvailable)
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Capability {
    MultiprotocolExtensions(MPExtVal),
    RouteRefreshPreStandard,
    RouteRefresh,
    EnhancedRouteRefresh,
    Extended4ByteASN(u32),
}

impl Capability {
    pub fn convert_to_bytes(&self) -> Result<Vec<u8>, MessageError> {
        let mut bytes = Vec::new();
        match self {
            Capability::Extended4ByteASN(as_num) => {
                bytes.extend_from_slice(&[0x02, 0x06, 0x41, 0x04]);
                // last 4 bytes are the 4 byte ASN
                let as4_bytes = bytes.extend_from_slice(&as_num.to_be_bytes());

            },
            Capability::MultiprotocolExtensions(mp_ext_val) => {
                bytes.extend_from_slice(&[0x02, 0x06, 0x01, 0x04]);
                match mp_ext_val {
                    MPExtVal::IPv4Unicast => {
                        bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
                    },
                    MPExtVal::IPv4Multicast => {
                        bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x02]);
                    },
                    MPExtVal::IPv4VPN => {
                        bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x80]);
                    },
                    MPExtVal::IPv6Unicast => {
                        bytes.extend_from_slice(&[0x02, 0x00, 0x00, 0x01]);
                    },
                    MPExtVal::IPv6Multicast => {
                        bytes.extend_from_slice(&[0x02, 0x00, 0x00, 0x02]);
                    },
                    MPExtVal::IPv6VPN => {
                        bytes.extend_from_slice(&[0x02, 0x00, 0x00, 0x80]);
                    },
                }
            },
            _ => {
                println!("Unimplemented match arm in Capability::convert_to_bytes()");
                return Err(MessageError::UnknownCapability)
            }

        }

        Ok(bytes)
    }
}

//
// I may come back and use the length field here so I'll keep this struct
#[derive(PartialEq, Debug, Clone)]
pub struct OptionalParameters {
    //param_type: u8,
    //param_length: u8,
    pub capabilities: Vec<Capability>, //variable length
}

impl OptionalParameters {
    pub fn new(
        multi_protocol_extensions_config: MultiProtocolExtensionsConfig,
        route_refresh_prestandard: bool, 
        route_refresh: bool, 
        enhanced_route_refresh: bool, 
        extended_4byte_asn: bool,
        as_num: Option<u32>) -> Self {
        
        let mut capabilities: Vec<Capability> = Vec::new();

        if multi_protocol_extensions_config.ipv4_multicast ||
            multi_protocol_extensions_config.ipv4_unicast ||
            multi_protocol_extensions_config.ipv4_vpn ||
            multi_protocol_extensions_config.ipv6_unicast ||
            multi_protocol_extensions_config.ipv6_multicast ||
            multi_protocol_extensions_config.ipv6_vpn
        {
            capabilities.push(Capability::MultiprotocolExtensions(MPExtVal::try_from(&multi_protocol_extensions_config).unwrap()));
        }

        if route_refresh_prestandard {
            capabilities.push(Capability::RouteRefreshPreStandard);
        }
        if route_refresh {
            capabilities.push(Capability::RouteRefresh);
        }
        if enhanced_route_refresh {
            capabilities.push(Capability::EnhancedRouteRefresh);
        }   
        if extended_4byte_asn && as_num.is_some() {
            capabilities.push(Capability::Extended4ByteASN(as_num.unwrap()));
        }
        
        OptionalParameters {
         capabilities    
        }
    }
}


pub fn is_4byte_asn_capability_present(capabilities: &Vec<Capability>) -> bool {
    for o in capabilities {
        match o {
            Capability::Extended4ByteASN(_) => return true,
            _ => {}
        }
    }

    false
}