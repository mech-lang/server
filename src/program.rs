// # Program

// # Prelude
extern crate ws;
extern crate bincode;

use std::sync::mpsc::{Sender, Receiver, SendError};
use std::thread::{self, JoinHandle};
use std::sync::mpsc;
use std::collections::{HashMap, HashSet, Bound, BTreeMap};
use std::mem;
use std::fs::{OpenOptions, File, canonicalize};
use std::io::{Write, BufReader, BufWriter};

use mech_core::{Core, Transaction, Change};
use mech_core::{Value};
use mech_core::Block;
use mech_core::{TableIndex, Hasher};
use mech_syntax::lexer::Lexer;
use mech_syntax::parser::{Parser, ParseStatus, Node};
use mech_syntax::compiler::Compiler;

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;
use time;

use watchers::{Watcher, WatchDiff};
use self::ws::{Sender as WSSender, Message};

// ## Program

pub struct Program {
  pub name: String,
  pub mech: Core,
  watchers: HashMap<u64, Box<Watcher + Send>>,
  pub out: WSSender,
  pub incoming: Receiver<RunLoopMessage>,
  pub outgoing: Sender<RunLoopMessage>,
}

impl Program {
  pub fn new(name:&str, out: WSSender, capacity: usize) -> Program {
    let (outgoing, incoming) = mpsc::channel();
    Program { 
      name: name.to_owned(), 
      mech: Core::new(capacity, 100),
      watchers: HashMap::new(),
      out,
      incoming, 
      outgoing 
    }
  }

  pub fn compile_string(&mut self, input: String) {
    let mut compiler = Compiler::new();
    compiler.compile_string(input);
    self.mech.runtime.register_blocks(compiler.blocks, &mut self.mech.store);
  }

}

// ## Run Loop

#[derive(Debug, Clone)]
pub enum RunLoopMessage {
  Stop,
  Pause,
  Resume,
  Clean,
  Reset,
  StepBack,
  StepForward,
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

// ## Persister

pub enum PersisterMessage {
    Stop,
    Write(Vec<Change>),
}

pub struct Persister {
    thread: JoinHandle<()>,
    outgoing: Sender<PersisterMessage>,
    loaded: Vec<Change>,
}

impl Persister {
  pub fn new(path_ref:&str) -> Persister {
    let (outgoing, incoming) = mpsc::channel();
    let path = path_ref.to_string();
    let thread = thread::spawn(move || {
      let file = OpenOptions::new().append(true).create(true).open(&path).unwrap();
      let mut writer = BufWriter::new(file);
      loop {
        match incoming.recv().unwrap() {
          PersisterMessage::Stop => { break; }
          PersisterMessage::Write(items) => {
            for item in items {
              let result = bincode::serialize(&item, bincode::Infinite).unwrap();
              if let Err(e) = writer.write_all(&result) {
                panic!("Can't persist! {:?}", e);
              }
            }
            writer.flush().unwrap();
          }
        }
      }
    });
    Persister { outgoing, thread, loaded: vec![] }
  }

  pub fn load(&mut self, path: &str) {
    let file = match File::open(path) {
      Ok(f) => f,
      Err(_) => {
        println!("Unable to load db: {}", path);
        return;
      }
    };
    let mut reader = BufReader::new(file);
    loop {
      let result:Result<Change, _> = bincode::deserialize_from(&mut reader, bincode::Infinite);
      match result {
        Ok(change) => {
          self.loaded.push(change);
        },
        Err(info) => {
          println!("ran out {:?}", info);
          break;
        }
      }
    }
  }

  pub fn send(&self, changes: Vec<Change>) {
    self.outgoing.send(PersisterMessage::Write(changes)).unwrap();
  }

  pub fn wait(self) {
    self.thread.join().unwrap();
  }

  pub fn get_channel(&self) -> Sender<PersisterMessage> {
    self.outgoing.clone()
  }

  pub fn get_changes(&mut self) -> Vec<Change> {
    mem::replace(&mut self.loaded, vec![])
  }

  pub fn close(&self) {
    self.outgoing.send(PersisterMessage::Stop).unwrap();
  }
}

// ## Program Runner

pub struct ProgramRunner {
  pub name: String,
  pub program: Program, 
  pub persistence_channel: Option<Sender<PersisterMessage>>,
}

impl ProgramRunner {

  pub fn new(name:&str, out: WSSender, capacity: usize) -> ProgramRunner {
    // Start a new program
    let mut program = Program::new(name, out, capacity);

    // Start a persister
    let persist_name = format!("{}.mdb", name);
    let mut persister = Persister::new(&persist_name);
    persister.load(&persist_name);
    let changes = persister.get_changes();

    // Intern the changes loaded into the persister
    for change in changes {
      program.mech.store.intern_change(&change);
    }

    ProgramRunner {
      name: name.to_owned(),
      program,
      // TODO Use the persistence file specified by the user
      persistence_channel: Some(persister.get_channel()),
    }
  }

  pub fn load_program(&mut self, input: String) {
    self.program.compile_string(input);
  }

  // TODO Move this out of program and into program runner
  pub fn attach_watcher(&mut self, watcher:Box<Watcher + Send>) {
    let name = Hasher::hash_str(&watcher.get_name());
    let columns = watcher.get_columns().clone();
    println!("{} {} #{}", &self.colored_name(), BrightGreen.paint("Loaded Watcher:"), &watcher.get_name());
    self.program.mech.register_watcher(name);
    self.program.watchers.insert(name, watcher);
    let watcher_table = Transaction::from_change(Change::NewTable{id: name, rows: 1, columns});
    self.program.outgoing.send(RunLoopMessage::Transaction(watcher_table));
  }

  pub fn add_persist_channel(&mut self, persister:&mut Persister) {
    self.persistence_channel = Some(persister.get_channel());
  }

  pub fn run(self) -> RunLoop {
    let name = self.colored_name();
    let outgoing = self.program.outgoing.clone();
    let mut program = self.program;
    let persistence_channel = self.persistence_channel;
     match persistence_channel {
      Some(ref channel) => {
        //channel.send(PersisterMessage::Write(to_persist));
      },
      _ => (),
    }
    println!("{} Applying stored changes...", name);
    let thread = thread::Builder::new().name(program.name.to_owned()).spawn(move || {
      println!("{} Starting run loop.", name);
      let mut paused = false;
      'runloop: loop {
        match (program.incoming.recv(), paused) {
          (Ok(RunLoopMessage::Transaction(txn)), false) => {
            println!("{} Txn started", name);
            let pre_changes = program.mech.store.len();
            let start_ns = time::precise_time_ns();
            program.mech.process_transaction(&txn);
            match persistence_channel {
              Some(ref channel) => {
                let mut to_persist: Vec<Change> = Vec::new();
                for i in program.mech.last_transaction .. program.mech.store.change_pointer {
                  let change = &program.mech.store.changes[i];
                  to_persist.push(change.clone());
                }
                channel.send(PersisterMessage::Write(to_persist));
              },
              _ => (),
            }
            // Handle watchers
            for (watcher_name, dirty) in program.mech.watched_index.iter_mut() {
              if *dirty {
                match program.watchers.get_mut(watcher_name) {
                  Some(watcher) => {
                    let mut diff = WatchDiff::new();
                    for i in program.mech.last_transaction .. program.mech.store.change_pointer {
                      let change = &program.mech.store.changes[i];
                      match change {
                        Change::Set{table, row, column, value} => {
                          if table == watcher_name {
                            diff.adds.push((*table, *row, *column, value.as_i64().unwrap()));
                          }
                        },
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
            let delta_changes = program.mech.store.len() - pre_changes;
            let end_ns = time::precise_time_ns();
            let time = (end_ns - start_ns) as f64;              
            // Send changes to connected clients
            // TODO Handle rollover of changes
            let mut adds: Vec<(u64,u64,u64,i64)> = Vec::new();
            let mut removes: Vec<(u64,u64,u64,i64)> = Vec::new();
            
            for i in program.mech.last_transaction .. program.mech.store.change_pointer {
              let change = &program.mech.store.changes[i];
              match change {
                Change::Set{table, row, column, value} => {
                  let i64_value = match value.as_i64() {
                    Some(n) => n,
                    None => 0,
                  };
                  // TODO this is a hack for now to send col ixes over to the client. In the future, we'll need to send the id->col mapping.
                  let column_ix: u64 = program.mech.store.tables.get(*table).unwrap().get_column_index(*column).unwrap().clone() as u64;
                  adds.push((*table, *row, column_ix, i64_value));
                },
                _ => (),
              }
            }
            let text = serde_json::to_string(&json!({"type": "diff", "adds": adds, "removes": removes, "client": program.name.clone()})).unwrap();
            program.out.send(Message::Text(text)).unwrap();
            //program.compile_string(String::from(text.clone()));
            //println!("{:?}", program.mech.store.changes);
            println!("{} Txn took {:0.4?} ms ({:0.0?} cps)", name, time / 1_000_000.0, delta_changes as f64 / (time / 1.0e9));
          },
          (Ok(RunLoopMessage::Stop), _) => break 'runloop,
          (Ok(RunLoopMessage::Pause), false) => paused = true,
          (Ok(RunLoopMessage::Resume), true) => paused = false,
          (Err(_), _) => break 'runloop,
          _ => (),
        }
      }
      if let Some(channel) = persistence_channel {
        channel.send(PersisterMessage::Stop);
      }
      println!("{} Run loop closed.", name);
    }).unwrap();
    RunLoop { thread, outgoing }
  }

  pub fn colored_name(&self) -> term_painter::Painted<String> {
    BrightCyan.paint(format!("[{}]", &self.name))
  }

}
