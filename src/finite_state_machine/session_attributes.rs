use crate::timers::Timer;
//
// moving this into the FSM struct
// #[derive(Debug, Default)]
// pub struct SessionAttributes {
//     pub allow_automatic_start: bool,
//     pub allow_automatic_stop: bool,
//     pub damp_peer_oscillations: bool,
//     pub idle_hold_time: u16, // initial value for timer
//     pub idle_hold_timer: Timer,
//     pub accept_connections_unconfigured_peers: bool,
//     pub passive_tcp_establishment: bool,
//     pub track_tcp_state: bool,
//     pub delay_open: bool,
//     pub delay_open_time: u16,
//     pub delay_open_timer: Timer,
//     pub send_notification_without_open: bool,
//     pub collision_detect_established_state: bool,
// }
