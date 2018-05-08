// # Program

// # Prelude

/*
use unicode_segmentation::UnicodeSegmentation;

use indexes::{HashIndex, DistinctIter, DistinctIndex, WatchIndex, IntermediateIndex, MyHasher, AggregateEntry,
              CollapsedChanges, RemoteIndex, RemoteChange, RawRemoteChange};
use solver::Solver;
use compiler::{make_block, parse_file, FunctionKind, Node};
use std::collections::{HashMap, HashSet, Bound, BTreeMap};
use std::mem::transmute;
use std::cmp::{self, Eq, PartialOrd};
use std::collections::hash_map::{DefaultHasher, Entry};
use std::hash::{Hash, Hasher};
use std::iter::{Iterator, FromIterator};
use std::fmt;
use watchers::{Watcher};


use serde::ser::{Serialize, Serializer};
use serde::de::{Deserialize, Deserializer, Visitor};
use std::error::Error;
use std::thread::{self, JoinHandle};
use std::io::{Write, BufReader, BufWriter};
use std::fs::{OpenOptions, File, canonicalize};
use std::path::{Path, PathBuf};
use std::f32::consts::{PI};
use std::mem;
use std::usize;
use rand::{Rng, SeedableRng, XorShiftRng};
use self::term_painter::ToStyle;
use self::term_painter::Color::*;
use parser;
use combinators::{ParseState, ParseResult};*/

extern crate time;

use std::sync::mpsc::{Sender, Receiver, SendError};
use std::thread::{self, JoinHandle};
use std::sync::mpsc;
use std::collections::{HashMap, HashSet, Bound, BTreeMap};

use mech::database::{Database, Transaction, Change};
use mech::table::{Value};

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;

use watchers::{Watcher};


// ## Program

pub struct Program {
  pub name: String,
  pub mech: Database,
  watchers: HashMap<String, Box<Watcher + Send>>,
  pub incoming: Receiver<RunLoopMessage>,
  pub outgoing: Sender<RunLoopMessage>,
}

impl Program {
  pub fn new(name:&str) -> Program {
    let (outgoing, incoming) = mpsc::channel();
    let mut db = Database::new(1000, 2);
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

  pub fn attach_watcher(&mut self, watcher:Box<Watcher + Send>) {
      let name = watcher.get_name();
      println!("{} {} {}", &self.colored_name(), BrightGreen.paint("Loaded Watcher:"), name);
      self.watchers.insert(name, watcher);
  }

  pub fn colored_name(&self) -> term_painter::Painted<String> {
    BrightCyan.paint(format!("[{}]", &self.name))
  }

}

// ## Run Loop

#[derive(Debug, Clone)]
pub enum RunLoopMessage {
  Stop,
  Pause,
  Resume,
  Transaction(Vec<(u64, u64, u64, u64)>),
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
  pub fn new(name:&str) -> ProgramRunner {
    ProgramRunner {
      name: name.to_owned(),
      program: Program::new(name),
    }
  }

  pub fn run(self) -> RunLoop {
    let outgoing = self.program.outgoing.clone();
    let mut program = self.program;
    let thread = thread::Builder::new().name(program.name.to_owned()).spawn(move || {
      println!("{} Starting run loop.", &program.colored_name());
      let mut paused = false;
      'outer: loop {
        match (program.incoming.recv(), paused) {
          (Ok(RunLoopMessage::Transaction(v)), false) => {
            println!("{} Txn started", &program.colored_name());
            let mut changes = vec![];
            for (table, row, col, value) in v {
              changes.push(Change::Add{table, row, column: col, value: Value::from_u64(value)})
            }    
            let txn = Transaction::from_changeset(changes);
            let start_ns = time::precise_time_ns();
            program.mech.process_transaction(&txn);
            let end_ns = time::precise_time_ns();
            let time = (end_ns - start_ns) as f64;
            println!("{:?}", program.mech);
            println!("{} Txn took {:0.4?} ms", &program.colored_name(), time / 1_000_000.0);
          },
          (Ok(m), _) => println!("{:?}", m),
          _ => (),
        }
      }
    }).unwrap();
    RunLoop { thread, outgoing }
  }

}
