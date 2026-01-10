
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


#[derive(PartialEq, Debug, Clone, Copy)]

pub struct MultiProtocolExtensionsCapability {
    // tbd
}




#[derive(PartialEq, Debug, Clone, Copy)]

pub enum Capability {
    MultiprotocolExtensions(MultiProtocolExtensionsCapability),
    RouteRefreshPreStandard,
    RouteRefresh,
    EnhancedRouteRefresh,
    Extended4ByteASN,

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
        route_refresh_prestandard: bool, 
        route_refresh: bool, 
        enhanced_route_refresh: bool, 
        extended_4byte_asn: bool) -> Self {
        
        let mut capabilities: Vec<Capability> = Vec::new();
        if route_refresh_prestandard {
            capabilities.push(Capability::RouteRefreshPreStandard);
        }
        if route_refresh {
            capabilities.push(Capability::RouteRefresh);
        }
        if enhanced_route_refresh {
            capabilities.push(Capability::EnhancedRouteRefresh);
        }   
        if extended_4byte_asn {
            capabilities.push(Capability::Extended4ByteASN);
        }
        
        OptionalParameters {
         capabilities    
        }
    }
}


