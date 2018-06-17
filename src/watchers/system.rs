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
    SystemTimerWatcher { name: "system/timer".to_string(), outgoing, timers: HashMap::new(), columns: 9 }
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
      if column == 1 {
        let outgoing = self.outgoing.clone();
        let system_timer = Hasher::hash_str(&self.get_name());
        let duration = Duration::from_millis(value as u64);
        thread::spawn(move || {
          let mut tick = 0;
          loop {
            thread::sleep(duration); 
            let cur_time = time::now();
            let txn = Transaction::from_changeset(vec![
              Change::Add{table, row, column: 2, value: Value::from_u64(cur_time.tm_year as u64 + 1900)},
              Change::Add{table, row, column: 4, value: Value::from_u64(cur_time.tm_mday as u64)},
              Change::Add{table, row, column: 3, value: Value::from_u64(cur_time.tm_mon as u64 + 1)},
              Change::Add{table, row, column: 5, value: Value::from_u64(cur_time.tm_hour as u64)},
              Change::Add{table, row, column: 6, value: Value::from_u64(cur_time.tm_min as u64)},
              Change::Add{table, row, column: 7, value: Value::from_u64(cur_time.tm_sec as u64)},
              Change::Add{table, row, column: 8, value: Value::from_u64(cur_time.tm_nsec as u64)},
              Change::Add{table, row, column: 9, value: Value::from_u64(tick)},
            ]);     
            tick += 1;
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
