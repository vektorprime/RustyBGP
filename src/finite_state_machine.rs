use std::io;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::errors::BGPError;
use crate::messages::{extract_messages_from_rec_data};
use crate::messages::open::get_neighbor_ipv4_address_from_stream;
use crate::process::BGPProcess;
use crate::timers::Timer;

use events::*;


pub mod events;
pub mod session_attributes;

#[derive(PartialEq, Default, Debug)]
pub enum State {
    #[default] Idle,
    Connect,
    Active,
    OpenSent,
    OpenConfirm,
    Established
}

// will need an FSM per neighbor
#[derive(Debug)]
pub struct FSM {
    pub state: State,
    pub connect_retry_counter: u16, // num of times we tried to establish a session
    pub connect_retry_time: u16, // suggested default is 120 from RFC
    pub connect_retry_timer: Timer, // uses the connect_retry_time for initial val
    pub hold_time: u16, // suggested default is 90
    pub hold_timer: Timer,
    pub keepalive_time: u16, // suggested default is 1/3 hold_time
    pub keepalive_timer: Timer,
    pub allow_automatic_start: bool,
    pub allow_automatic_stop: bool,
    pub damp_peer_oscillations: bool,
    pub idle_hold_time: u16, // initial value for timer
    pub idle_hold_timer: Timer,
    pub accept_connections_unconfigured_peers: bool,
    pub passive_tcp_establishment: bool,
    pub track_tcp_state: bool,
    pub delay_open: bool,
    pub delay_open_time: u16,
    pub delay_open_timer: Timer,
    pub send_notification_without_open: bool,
    pub collision_detect_established_state: bool,
}

impl Default for FSM {
    fn default() -> Self {
        FSM {
            state: State::Idle,
            connect_retry_counter: 0,
            connect_retry_time: 120,
            connect_retry_timer: Timer::new(120),
            hold_time: 90,
            hold_timer: Timer::new(90),
            keepalive_time: 30,
            keepalive_timer: Timer::new(30),
            // TODO refactor to get these values from config, we'll temp set to this for now
            allow_automatic_start: true,
            allow_automatic_stop: true,
            damp_peer_oscillations: false,
            idle_hold_time: 30,
            idle_hold_timer: Timer::new(30),
            accept_connections_unconfigured_peers: false,
            passive_tcp_establishment: true,
            track_tcp_state: true,
            delay_open: true,
            delay_open_time: 5,
            delay_open_timer: Timer::new(5),
            send_notification_without_open: false,
            collision_detect_established_state: true,
        }
    }
}

impl FSM {


    pub fn run(fsm: &mut FSM, bgp_proc: Arc<Mutex<BGPProcess>>, tcp_stream: TcpStream) {

    }

    // //
    // pub fn set_state(&mut self, state: State) -> Result<(), BGPError> {
    //     match state {
    //         State::Idle => {
    //             self.connect_retry_counter = 0;
    //             self.connect_retry_timer = Some(Timer::new(self.connect_retry_time));
    //             self.state = State::Idle;
    //             Ok(())
    //         },
    //         State::Connect => {
    //
    //
    //             self.connect_retry_counter = 0;
    //             self.connect_retry_timer = Some(Timer::new(self.connect_retry_time));
    //             // initiate tcp conn to peer and/or listen for a tcp conn
    //
    //
    //             self.state = State::Connect;
    //             Ok(())
    //         },
    //         State::Active => {
    //
    //
    //             self.state = State::Active;
    //             Ok(())
    //         },
    //         State::OpenSent => {
    //
    //
    //             self.state = State::OpenSent;
    //             Ok(())
    //         },
    //         State::OpenConfirm => {
    //
    //
    //             self.state = State::OpenConfirm;
    //             Ok(())
    //         },
    //         State::Established => {
    //
    //
    //             self.state = State::Established;
    //             Ok(())
    //         }
    //     }
    // }
}

// impl Default for FSM {
//     fn default() -> Self {
//         FSM::Idle
//     }
// }