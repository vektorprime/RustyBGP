use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::errors::EventError;
use crate::routes::{RouteV4, NLRI};

#[derive(Debug)]
pub struct NeighborChannel {
    pub tx: mpsc::Sender<ChannelMessage>,
    pub rx: mpsc::Receiver<ChannelMessage>,
    //pub is_active: bool,
}


pub enum TCPChannelMessage {
    DropTCP
}
pub enum ChannelMessage {
    Route(RouteV4),
    WithdrawRoute(Vec<NLRI>),
    //NeighborDown,
    NeighborUp,
    TcpEstablished(TcpStream)
}

impl NeighborChannel {
    pub async fn bring_up(&mut self) -> Result<(), EventError> {
        //self.is_active = true;
        self.tx.send(ChannelMessage::NeighborUp).await.map_err(|_| EventError::ChannelDown)?;
        Ok(())
    }

    // pub async fn take_down(&mut self) -> Result<(), EventError> {
    //     self.is_active = false;
    //     self.tx.send(ChannelMessage::NeighborDown).await.map_err(|_| EventError::ChannelDown)?;
    //     Ok(())
    // }

    pub async fn send_route(&self, route: RouteV4) {
        //if self.is_active {
            self.tx.send(ChannelMessage::Route(route)).await.unwrap();
        //}
    }


    pub async fn send_route_vec(&self, routes: &Vec<RouteV4>) {
        //if self.is_active {
        for route in routes {
            self.tx.send(ChannelMessage::Route(route.clone())).await.unwrap();
        }
        
        //}
    }


    pub async fn withdraw_route(&self, nlri_vec: Vec<NLRI>) {
        //if self.is_active {
            self.tx.send(ChannelMessage::WithdrawRoute(nlri_vec)).await.unwrap();
        //}
    }

    pub fn recv_tcp_conn_from_bgp_proc(&mut self) -> Option<TcpStream> {
        //println!("executing recv_tcp_conn_from_bgp_proc");
        if let Ok(ChannelMessage::TcpEstablished(tcp_stream)) = self.rx.try_recv() {
            return Some(tcp_stream)
        }
        None
    }

    pub fn send_tcp_conn_to_neighbor(&self, tcp_stream: TcpStream) -> Result<(), EventError> {
        //println!("executing send_tcp_conn_to_neighbor");
        self.tx.try_send(ChannelMessage::TcpEstablished(tcp_stream)).map_err(|_| EventError::ChannelDown)
    }
}