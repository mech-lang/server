extern crate time;
use std::time::Duration;
use super::{Watcher, WatchDiff};
use super::super::program::{RunLoopMessage};
use mech::{Interner, Transaction, Change};
use mech::Hasher;
use mech::Value;
use std::sync::mpsc::{self, Sender};
use std::thread::{self};
use std::collections::{HashMap};
use std::collections::hash_map::{Entry};

pub struct SystemTimerWatcher {
  name: String,
  columns: usize,
  outgoing: Sender<RunLoopMessage>,
  timers: HashMap<u64, (usize, Sender<()>)>
}

impl SystemTimerWatcher {
  pub fn new(outgoing: Sender<RunLoopMessage>) -> SystemTimerWatcher {
    SystemTimerWatcher { name: "system/timer".to_string(), outgoing, timers: HashMap::new(), columns: 10 }
  }
}

impl Watcher for SystemTimerWatcher {
  fn get_name(& self) -> String {
    self.name.clone()
  }
  fn set_name(&mut self, name: &str) {
    self.name = name.to_string();
  }
  fn get_columns(&self) -> usize {
    self.columns
  }
  fn on_diff(&mut self, interner: &mut Interner, diff: WatchDiff) {
    for remove in diff.removes {

    }
    for (table, row, column, value) in diff.adds {
      if column == Hasher::hash_str("resolution") {
        let outgoing = self.outgoing.clone();
        let system_timer = Hasher::hash_str(&self.get_name());
        let duration = Duration::from_millis(value as u64);
        thread::spawn(move || {
          let mut tick = 0;
          let mut last = 0;
          loop {
            thread::sleep(duration); 
            let cur_time = time::now();
            let now = time::precise_time_ns();
            let txn = Transaction::from_changeset(vec![
              Change::Add{table, row, column: Hasher::hash_str("year"), value: Value::from_u64(cur_time.tm_year as u64 + 1900)},
              Change::Add{table, row, column: Hasher::hash_str("day"), value: Value::from_u64(cur_time.tm_mday as u64)},
              Change::Add{table, row, column: Hasher::hash_str("month"), value: Value::from_u64(cur_time.tm_mon as u64 + 1)},
              Change::Add{table, row, column: Hasher::hash_str("hour"), value: Value::from_u64(cur_time.tm_hour as u64)},
              Change::Add{table, row, column: Hasher::hash_str("minute"), value: Value::from_u64(cur_time.tm_min as u64)},
              Change::Add{table, row, column: Hasher::hash_str("second"), value: Value::from_u64(cur_time.tm_sec as u64)},
              Change::Add{table, row, column: Hasher::hash_str("nano-second"), value: Value::from_u64(cur_time.tm_nsec as u64)},
              Change::Add{table, row, column: Hasher::hash_str("tick"), value: Value::from_u64(tick)},
              Change::Add{table, row, column: Hasher::hash_str("dt"), value: Value::from_u64(now - last)},
            ]);     
            tick += 1;
            last = now;
            match outgoing.send(RunLoopMessage::Transaction(txn)) {
              Err(_) => break,
              _ => {}
            }
          }
        });
      }
    }  
  }
}
