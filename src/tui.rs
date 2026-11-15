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
use ratatui::widgets::{Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap};
use tokio::sync::broadcast::channel;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::process::RIB;

pub fn init_tui(log_rx: Receiver<String>) -> Sender<RIB> {
    let (tx_tui, rx_tui) = mpsc::channel::<RIB>(2);
    let mut tui = Tui::new(log_rx, rx_tui);
    tokio::spawn(async move {
        ratatui::run(|terminal| tui.run_loop(terminal));
    });
    tx_tui
}

enum ScrollBarDirection {
    Up,
    Down,
}

#[derive(Debug)]
enum CurrentView {
    Logs,
    Routes,
}

#[derive(Debug)]
pub struct Tui {
    pub quit: bool,
    pub rx_log: Receiver<String>,
    pub log_history: VecDeque<String>,
    pub scroll_position: usize,
    pub scroll_state: ScrollbarState,
    pub scroll_text_len: usize,
    pub auto_scroll: bool,
    pub rx_tui: Receiver<RIB>,
    pub rib: Option<RIB>,
    pub current_view: CurrentView,
    pub table_position: usize,
}

impl Tui {
    pub fn new(rx_log: Receiver<String>, rx_tui: Receiver<RIB>) -> Self {
        let mut log_history: Vec<String> = Vec::new();
        log_history.reserve(65536);
        Tui {
            quit: false,
            rx_log,
            log_history: VecDeque::new(),
            scroll_position: 0,
            scroll_state: ScrollbarState::new(0),
            scroll_text_len: 0,
            auto_scroll: true,
            rx_tui,
            rib: None,
            current_view: CurrentView::Logs,
            table_position: 0,
        }
    }
    pub fn run_loop(&mut self, terminal: &mut DefaultTerminal)  {
        while !self.quit {
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

            match self.rx_tui.try_recv() {
                Ok(rib) => {
                    self.rib = Some(rib);
                    log::info!("Received RIB in rx_tui.try_recv()");
                }
                Err(e) => {

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
                log::info!("SHIFT + TAB detected");
                // TODO refactor this to use some sort of next() func
                match self.current_view {
                    CurrentView::Logs => { self.current_view = CurrentView::Routes},
                    CurrentView::Routes => { self.current_view = CurrentView::Logs}
                }
            },
            KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                //log::info!("CTRL + Q detected");
                self.quit = true;
            }
            KeyCode::Left => {

            },
            KeyCode::Right => {

            },
            KeyCode::Up => {
                //log::info!("Up arrow detected");
                self.scroll(5, ScrollBarDirection::Up);
            },
            KeyCode::Down => {
                //log::info!("Down arrow detected");
                self.scroll(5, ScrollBarDirection::Down);
            },
            KeyCode::Char('s') => {
                //log::info!("CTRL + Q detected");
                log::info!("S detected");
                log::info!("toggling auto-scroll");
                if self.auto_scroll {
                    self.auto_scroll = false;
                }
                else {
                    self.auto_scroll = true;
                }

            }
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
        let mut title = Line::from(" Rusty BGP ".bold());

        let inst_auto_scroll = if self.auto_scroll {
            "<S> ".green().bold()
        } else {
            "<S> ".red().bold()
        };

        let instructions = Line::from(vec![
            " Decrement ".into(),
            "<Left>".blue().bold(),
            " Increment ".into(),
            "<Right>".blue().bold(),
            " Switch View ".into(),
            "<SHIFT + TAB>".blue().bold(),
            " Quit ".into(),
            "<CTRL + Q>".blue().bold(),
            " Auto-Scroll ".into(),
            inst_auto_scroll,
        ]);

        // let block = Block::bordered()
        //     .title(title.centered())
        //     .title_bottom(instructions.centered())
        //     .border_set(border::THICK);

        match self.current_view {
            CurrentView::Logs => {
                let mut title = Line::from(" Rusty BGP - Logs ".bold());

                let block = Block::bordered()
                    .title(title.centered())
                    .title_bottom(instructions.centered())
                    .border_set(border::THICK);

                let mut vec_text = Vec::new();
                for entry in &self.log_history {
                    vec_text.push(Line::from(entry.clone()));
                };

                // TODO determine if we'll keep unstable feat.
                // I'm not sure if I want to keep unstable features just for getting the scroll line correct, I was only off by a few
                //let vec_text_len = vec_text.len();
                //let auto_scroll_position = self.log_history.len().saturating_sub(area.height as usize);
                // let paragraph =  if self.auto_scroll {
                //     Paragraph::new(vec_text)
                //         .left_aligned()
                //         .block(block)
                //         .wrap(Wrap{ trim: true })
                //         .scroll((auto_scroll_position as u16, 0))
                // } else {
                //     Paragraph::new(vec_text)
                //         .left_aligned()
                //         .block(block)
                //         .wrap(Wrap{ trim: true })
                //         .scroll((self.scroll_position as u16, 0))
                // };

                let mut paragraph = Paragraph::new(vec_text)
                    .left_aligned()
                    .block(block)
                    .wrap(Wrap{ trim: true });
                let auto_scroll_position = paragraph.line_count(area.width.saturating_sub(2))
                    .saturating_sub(area.height as usize);
                if self.auto_scroll {
                    paragraph = paragraph.scroll((auto_scroll_position as u16, 0));
                } else {
                    paragraph = paragraph.scroll((self.scroll_position as u16, 0));
                }
                // log::info!("{}",area.height);

                paragraph.render(area, buf);

                let mut state = if self.auto_scroll {
                    ScrollbarState::new(self.log_history.len()).position(auto_scroll_position)
                } else {
                    ScrollbarState::new(self.log_history.len()).position(self.scroll_position)
                };
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

                StatefulWidget::render(
                    scrollbar,
                    area.inner(Margin { vertical: 1, horizontal: 0}),
                    buf,
                    &mut state,
                );
            },
            CurrentView::Routes => {
                // let mut vec_text = Vec::new();
                // for entry in &self.log_history {
                //     vec_text.push(Line::from(entry.clone()));
                // };
                // let mut paragraph = Paragraph::new(vec_text)
                //     .left_aligned()
                //     .block(block)
                //     .wrap(Wrap{ trim: true });
                let mut title = Line::from(" Rusty BGP - Routes ".bold());

                let block = Block::bordered()
                    .title(title.centered())
                    .title_bottom(instructions.centered())
                    .border_set(border::THICK);

                //let header = Row::new(vec!["Prefix", "Next Hop", "Local Pref", "Origin", "AS Path", "MED"]);
                let header = Row::new(vec!["Prefix", "Next Hop"]);
                let mut rows = Vec::new();
                if let Some(rib) = &self.rib {
                    for (nlri, routes) in &rib.routes {
                            rows.push(Row::new(vec![nlri.prefix.to_string(), nlri.len.to_string()]));
                    }
                }
                let widths = [Constraint::Length(5), Constraint::Length(5)];
                let table = Table::new(rows, widths).block(block).header(header);
                let mut table_state = TableState::default().with_selected(self.table_position);
                StatefulWidget::render(
                  table,
                  area.inner(Margin { vertical: 1, horizontal: 0}),
                  buf,
                  &mut table_state
                );
            }
        }



    }


}