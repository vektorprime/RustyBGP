use std::collections::VecDeque;
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
        ratatui::run(|terminal| tui.run_loop(terminal));
    });
}

#[derive(Debug)]
pub struct Tui {
    exit: bool,
    rx_log: Receiver<String>,
    log_history: VecDeque<String>
}

impl Tui {
    pub fn new(rx_log: Receiver<String>) -> Self {
        let mut log_history: Vec<String> = Vec::new();
        log_history.reserve(65536);
        Tui {
            exit: false,
            rx_log,
            log_history: VecDeque::new()
        }
    }
    pub fn run_loop(&mut self, terminal: &mut DefaultTerminal)  {
        while !self.exit {
            match self.rx_log.try_recv() {
                Ok(rx_msg) => {
                    // rotate the log
                    if self.log_history.len() == 65535 {
                        // don't need the popped item
                        self.log_history.pop_front();
                    }
                    self.log_history.push_back(rx_msg.clone());
                }
                Err(e) => {
                    //println!("Error in TUI logger");
                }
            }

            terminal.draw(|frame| self.draw(frame));
            // if a new log comes in append it
            // add a button to auto-refresh logs
            // add a button to switch to log view
        }
        //Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

}

impl Widget for &Tui {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Rusty BGP ".bold());
        let instructions = Line::from(vec![
            " Decrement ".into(),
            "<Left>".blue().bold(),
            " Increment ".into(),
            "<Right>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let mut text = String::new();
        for entry in &self.log_history {
            text = text + entry + "\n";
        }
        let mut vec_text = Vec::new();
        for entry in &self.log_history {
            vec_text.push(Line::from(entry.clone()));
        };

        Paragraph::new(vec_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}