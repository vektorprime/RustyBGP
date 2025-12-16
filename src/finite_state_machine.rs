use crate::errors::BGPError;
use crate::timers::Timer;

#[derive(Default, Debug)]
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
    state: State,
    connect_retry_counter: u16, // num of times we tried to establish a session
    connect_retry_time: u16, // suggested default is 120 from RFC
    connect_retry_timer: Option<Timer>, // uses the connect_retry_time for initial val
    hold_time: u16, // suggested default is 90
    hold_timer: Option<Timer>,
    keepalive_time: u16, // suggested default is 1/3 hold_time
    keepalive_timer: Option<Timer>,
}

impl Default for FSM {
    fn default() -> Self {
        FSM {
            state: State::Idle,
            connect_retry_counter: 0,
            connect_retry_time: 120,
            connect_retry_timer: None,
            hold_time: 90,
            hold_timer: None,
            keepalive_time: 30,
            keepalive_timer: None,
        }
    }
}

impl FSM {
    // TODO finish state machine
    pub fn set_state(&mut self, state: State) -> Result<(), BGPError> {
        match state {
            State::Idle => {
                // ManualStop and AutomaticStop are ignored in Idle

                self.state = State::Idle;
                Ok(())
            },
            State::Connect => {

                // TODO ManualStart and AutomaticStart events should trigger this
                self.connect_retry_counter = 0;
                self.connect_retry_timer = Some(Timer::new(self.connect_retry_time));
                // initiate tcp conn to peer and/or listen for a tcp conn


                self.state = State::Connect;
                Ok(())
            },
            State::Active => {


                self.state = State::Active;
                Ok(())
            },
            State::OpenSent => {


                self.state = State::OpenSent;
                Ok(())
            },
            State::OpenConfirm => {


                self.state = State::OpenConfirm;
                Ok(())
            },
            State::Established => {


                self.state = State::Established;
                Ok(())
            }
        }
    }
}

// impl Default for FSM {
//     fn default() -> Self {
//         FSM::Idle
//     }
// }