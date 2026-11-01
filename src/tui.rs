use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use tokio::sync::mpsc::{Receiver};



pub fn init_tui(log_rx: Receiver<String>) {
    let mut tui = Tui::new(log_rx);
    tokio::spawn(async move {

        loop {
            match &tui.rx_log.try_recv() {
                Ok(rx_msg) => {
                    println!("LOG - {}", rx_msg);
                }
                Err(e) => {
                    //println!("Error in TUI logger");
                }
            }

        }

    });
}

pub struct Tui {
    exit: bool,
    rx_log: Receiver<String>
}

impl Tui {
    pub fn new(rx_log: Receiver<String>) -> Self {
        Tui {
            exit: false,
            rx_log
        }
    }

}