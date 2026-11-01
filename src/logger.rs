use tokio::sync::mpsc::{Sender};
use log::{Log, Metadata, Record, Level};

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