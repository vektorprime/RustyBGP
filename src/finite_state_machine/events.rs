
// optional
// 2 groups
// group 1 automatic administrative events
// group 2 unconfigured peers

// pub enum EventType {
//     Optional,
//     Mandatory
// }

// I'll try these as a single struct added to the neighbor
// pub enum OptionalAttribute {
//     AllowAutomaticStart(bool),
//     AllowAutomaticStop(bool),
//     DampPeerOscillations(bool),
//     IdleHoldTime(u16),
//     IdleHoldTimer(u16),
//     AcceptConnectionsUnconfiguredPeers(bool),
//     PassiveTcpEstablishment(bool),
//     TrackTcpState(bool),
//     DelayOpen(bool),
//     DelayOpenTime(u16),
//     DelayOpenTimer(u16),
//     SendNotificationWithoutOpen(bool),
//     CollisionDetectEstablishedState(bool),
// }

use crate::messages::keepalive::KeepaliveMessage;
use crate::messages::notification::NotificationMessage;
use crate::messages::open::OpenMessage;
use crate::messages::route_refresh::RouteRefreshMessage;
use crate::messages::update::UpdateMessage;

#[derive(Debug, Clone)]
pub enum Event {
    // Administrative events
    ManualStart,
    ManualStop,
    AutomaticStart,
    ManualStartWithPassiveTcpEstablishment,
    AutomaticStartWithPassiveTcpEstablishment,
    AutomaticStartWithDampPeerOscillations,
    AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment,
    AutomaticStop,
    // Timer events
    ConnectRetryTimerExpires,
    HoldTimerExpires,
    KeepaliveTimerExpires,
    DelayOpenTimerExpires,
    IdleHoldTimerExpires,
    // TCP connection events
    TcpConnectionValid,
    TcpCRInvalid,
    TcpCRAcked,
    TcpConnectionConfirmed,
    TcpConnectionFails,
    // BGP message events
    OpenMsg(OpenMessage),
    BGPOpenWithDelayOpenTimerRunning,
    BGPHeaderErr,
    BGPOpenMsgErr,
    OpenCollisionDump,
    NotifMsgVerErr,
    NotifMsg(NotificationMessage),
    KeepAliveMsg,
    UpdateMsg(UpdateMessage),
    RouteRefreshMsg(RouteRefreshMessage), // custom event I added to handle the message type
    UpdateMsgErr


}



