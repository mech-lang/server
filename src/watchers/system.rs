extern crate time;
use std::time::Duration;
use super::{Watcher, WatchDiff};
use super::super::program::{RunLoopMessage};
use mech::database::{Interner, Transaction, Change};
use mech::indexes::Hasher;
use mech::table::Value;
use std::sync::mpsc::{self, Sender};
use std::thread::{self};
use std::collections::{HashMap};
use std::collections::hash_map::{Entry};

pub struct SystemTimerWatcher {
  name: String,
  outgoing: Sender<RunLoopMessage>,
  timers: HashMap<u64, (usize, Sender<()>)>
}

impl SystemTimerWatcher {
  pub fn new(outgoing2: Sender<RunLoopMessage>) -> SystemTimerWatcher {
      let outgoing = outgoing2.clone();
      let system_timer_change = Hasher::hash_str("system/timer/change");
      thread::spawn(move || {
        let mut tick = 0;
        let txn = Transaction::from_changeset(
        vec![
          Change::NewTable{tag: system_timer_change, rows: 1, columns: 4}
        ]); 
        outgoing.send(RunLoopMessage::Transaction(txn));
        loop {
          thread::sleep(Duration::from_millis(10)); 
          let cur_time = time::now();
          let txn = Transaction::from_changeset(vec![
            Change::Add{table: system_timer_change, row: 1, column: 1, value: Value::from_u64(cur_time.tm_hour as u64)},
            Change::Add{table: system_timer_change, row: 1, column: 2, value: Value::from_u64(cur_time.tm_min as u64)},
            Change::Add{table: system_timer_change, row: 1, column: 3, value: Value::from_u64(cur_time.tm_sec as u64)},
            Change::Add{table: system_timer_change, row: 1, column: 4, value: Value::from_u64(cur_time.tm_nsec as u64)},
          ]);     
          tick += 1;
          match outgoing.send(RunLoopMessage::Transaction(txn)) {
            Err(_) => break,
            _ => {}
          }
        }
      });
    SystemTimerWatcher { name: "system/timer".to_string(), outgoing: outgoing2, timers: HashMap::new() }
  }
}

impl Watcher for SystemTimerWatcher {
  fn get_name(& self) -> String {
    self.name.clone()
  }
  fn set_name(&mut self, name: &str) {
    self.name = name.to_string();
  }

  fn on_diff(&mut self, interner: &mut Interner, diff: WatchDiff) {

    for remove in diff.removes {

    }

    for add in diff.adds {
      
    }

    

  }
}
