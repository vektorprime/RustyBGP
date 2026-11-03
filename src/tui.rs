use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
use ratatui::layout::{Alignment, Constraint, Layout, Margin};
use ratatui::prelude::StatefulWidget;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use tokio::sync::mpsc::{Receiver};



pub fn init_tui(log_rx: Receiver<String>) {
    let mut tui = Tui::new(log_rx);
    tokio::spawn(async move {
        ratatui::run(|terminal| tui.run_loop(terminal));
    });
}

enum ScrollBarDirection {
    Up,
    Down,
}

#[derive(Debug)]
pub struct Tui {
    pub exit: bool,
    pub rx_log: Receiver<String>,
    pub log_history: VecDeque<String>,
    pub scroll_position: usize,
    pub scroll_state: ScrollbarState,
    pub scroll_text_len: usize,
}

impl Tui {
    pub fn new(rx_log: Receiver<String>) -> Self {
        let mut log_history: Vec<String> = Vec::new();
        log_history.reserve(65536);
        Tui {
            exit: false,
            rx_log,
            log_history: VecDeque::new(),
            scroll_position: 0,
            scroll_state: ScrollbarState::new(0),
            scroll_text_len: 0,
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

            terminal.draw(|frame| self.draw_tui(frame));
            // if a new log comes in append it
            // add a button to auto-refresh logs
            // add a button to switch to log view
            self.handle_event();
        }
        //Ok(())
    }

    fn draw_tui(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());

    }

    fn handle_event(&mut self) {
        //log::info!("in handle_event");
        if let Ok(true) = event::poll(Duration::from_millis(100)) {
            //log::info!("event:poll is true");
            match event::read()  {
                Ok(event)  => {
                    //log::info!("event::read() returned Ok(event)");
                    match event {
                        Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                            //log::info!("Got a key_event");
                            self.handle_key_event(key_event)
                        },
                        _ => {
                            //log::info!("unhandled event");
                        }
                    } // end match event
                },
                Err(e) => {
                    log::error!("Error in match event::read()");
                }

                _ => {}
            } // end match event::read()
        }

    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        //log::info!("in handle_key_event");
        match key_event.code {
            KeyCode::BackTab => {
                // TODO switch view
                log::info!("SHIFT + TAB detected");
            },
            KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                log::info!("CTRL + Q detected");
            }
            KeyCode::Left => {

            },
            KeyCode::Right => {

            },
            KeyCode::Up => {
                log::info!("Up arrow detected");
                self.scroll(5, ScrollBarDirection::Up);
            },
            KeyCode::Down => {
                log::info!("Down arrow detected");
                self.scroll(5, ScrollBarDirection::Down);
            },
            _ => {}
        }
    }



    fn scroll(&mut self, scroll_size: usize, scroll_direction: ScrollBarDirection) {
        match scroll_direction {
            ScrollBarDirection::Up => {
                self.scroll_position = self.scroll_position.saturating_sub(scroll_size);
            },
            ScrollBarDirection::Down => {
                self.scroll_position = self.scroll_position.saturating_add(scroll_size);
            }
        }
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
            " Switch View".into(),
            " <SHIFT + TAB>".blue().bold(),
            " Quit ".into(),
            "<CTRL + Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);



        let mut text = String::new();
        for entry in &self.log_history {
            text = text + entry + "\n";
        }
        let mut text_len = 0;
        let mut vec_text = Vec::new();
        for entry in &self.log_history {
            text_len += entry.len();
            vec_text.push(Line::from(entry.clone()));
        };

        let vec_text_len = vec_text.len();
        Paragraph::new(vec_text)
            .left_aligned()
            .block(block)
            .scroll((self.scroll_position as u16, 0))
            .render(area, buf);

        let mut state = ScrollbarState::new(vec_text_len).position(self.scroll_position);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

        StatefulWidget::render(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0}),
            buf,
            &mut state,
        );


        //self.render_vertical_scroll(&vec_text, &chunks)
    }


}