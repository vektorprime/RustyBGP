#[derive(Default)]
pub enum FSM {
    #[default] Idle,
    Connect,
    Active,
    OpenSent,
    OpenConfirm,
    Established
}

// impl Default for FSM {
//     fn default() -> Self {
//         FSM::Idle
//     }
// }