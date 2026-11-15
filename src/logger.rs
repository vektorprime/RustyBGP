use tokio::sync::mpsc::{Receiver, Sender};
use log::{Log, Metadata, Record, Level};
use tokio::sync::mpsc;
use crate::tui;

pub struct BGPLogger {
    pub tx: Sender<String>
}



impl Log for BGPLogger {
   fn enabled(&self, metadata: &Metadata) -> bool {
       metadata.level() <= Level::Info
   }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = self.tx.try_send(format!("{} - {}", record.level(), record.args()));
        }
    }
    fn flush(&self) {}
}

impl BGPLogger {
    pub fn setup_logger() -> Receiver<String> {
        let (log_tx, log_rx) = mpsc::channel::<String>(255);
        let logger = BGPLogger { tx: log_tx.clone() };
        log::set_boxed_logger(Box::new(logger)).unwrap();
        log::set_max_level(log::LevelFilter::Debug);
        //tui::init_tui(log_rx);
        log_rx
    }
}