// # Program

// # Prelude

extern crate time;

use std::sync::mpsc::{Sender, Receiver, SendError};
use std::thread::{self, JoinHandle};
use std::sync::mpsc;
use std::collections::{HashMap, HashSet, Bound, BTreeMap};

use mech::database::{Database, Transaction, Change};
use mech::table::{Value};
use mech::indexes::{TableIndex, Hasher};

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;

use watchers::{Watcher, WatchDiff};

// ## Program

pub struct Program {
  pub name: String,
  pub mech: Database,
  watchers: HashMap<u64, Box<Watcher + Send>>,
  pub incoming: Receiver<RunLoopMessage>,
  pub outgoing: Sender<RunLoopMessage>,
}

impl Program {
  pub fn new(name:&str, capacity: usize) -> Program {
    let (outgoing, incoming) = mpsc::channel();
    let mut db = Database::new(capacity, 100);
    let mut table_changes = vec![
      Change::NewTable{tag: 1, rows: 2, columns: 4}, 
    ];
    let txn = Transaction::from_changeset(table_changes);
    db.process_transaction(&txn);
    Program { 
      name: name.to_owned(), 
      mech: db,
      watchers: HashMap::new(),
      incoming, 
      outgoing 
    }
  }
}

// ## Run Loop

#[derive(Debug, Clone)]
pub enum RunLoopMessage {
  Stop,
  Pause,
  Resume,
  Transaction(Transaction),
}

pub struct RunLoop {
  thread: JoinHandle<()>,
  outgoing: Sender<RunLoopMessage>,
}

impl RunLoop {
  pub fn wait(self) {
    self.thread.join().unwrap();
  }

  pub fn close(&self) {
    match self.outgoing.send(RunLoopMessage::Stop) {
      Ok(..) => (),
      Err(..) => (),
    }
  }

  pub fn send(&self, msg: RunLoopMessage) {
    self.outgoing.send(msg).unwrap();
  }

  pub fn channel(&self) -> Sender<RunLoopMessage> {
    self.outgoing.clone()
  }
}

// ## Program Runner

pub struct ProgramRunner {
  pub name: String,
  pub program: Program,  
}

impl ProgramRunner {

  pub fn new(name:&str, capacity: usize) -> ProgramRunner {
    ProgramRunner {
      name: name.to_owned(),
      program: Program::new(name, capacity),
    }
  }

  // TODO Move this out of program and into program runner
  pub fn attach_watcher(&mut self, watcher:Box<Watcher + Send>) {
    let name = Hasher::hash_str(&watcher.get_name());
    println!("{} {} #{}", &self.colored_name(), BrightGreen.paint("Loaded Watcher:"), &watcher.get_name());
    self.program.mech.register_watcher(name);
    self.program.watchers.insert(name, watcher);
  }


  pub fn run(self) -> RunLoop {
    let name = self.colored_name();
    let outgoing = self.program.outgoing.clone();
    let mut program = self.program;
    let thread = thread::Builder::new().name(program.name.to_owned()).spawn(move || {
      println!("{} Starting run loop.", name);
      let mut paused = false;
      'outer: loop {
        match (program.incoming.recv(), paused) {
          (Ok(RunLoopMessage::Transaction(txn)), false) => {
            //println!("{} Txn started", name);
            let start_ns = time::precise_time_ns();
            program.mech.process_transaction(&txn);
            // Process watchers
            for (watcher_name, dirty) in program.mech.watched_index.iter_mut() {
              if *dirty {
                match program.watchers.get_mut(watcher_name) {
                  Some(watcher) => {
                    let mut diff = WatchDiff::new();
                    for i in program.mech.last_transaction .. program.mech.store.change_pointer {
                      let change = &program.mech.store.changes[i];
                      match change {
                        Change::Add{table, ..} => {
                          if table == watcher_name {
                            diff.adds.push(change.clone());
                          }
                        }
                        _ => (),
                      }
                    }
                    watcher.on_diff(&mut program.mech.store, diff);
                    *dirty = false;
                  },
                  _ => (),
                };
              }
            }
            let end_ns = time::precise_time_ns();
            let time = (end_ns - start_ns) as f64;
            //println!("{:?}", program.mech);
            //println!("{} Txn took {:0.4?} ms", name, time / 1_000_000.0);
          },
          (Ok(m), _) => println!("{:?}", m),
          _ => (),
        }
      }
    }).unwrap();
    RunLoop { thread, outgoing }
  }

  pub fn colored_name(&self) -> term_painter::Painted<String> {
    BrightCyan.paint(format!("[{}]", &self.name))
  }

}
