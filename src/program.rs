// # Program

// # Prelude

use std::sync::mpsc::{Sender, Receiver, SendError};
use std::thread::{self, JoinHandle};
use std::sync::mpsc;
use std::collections::{HashMap, HashSet, Bound, BTreeMap};

use mech::{Core, Transaction, Change};
use mech::{Value};
use mech::Block;
use mech::{TableIndex, Hasher};
use mech_syntax::lexer::Lexer;
use mech_syntax::parser::{Parser, ParseStatus, Node};
use mech_syntax::compiler::Compiler;

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;
use time;

use watchers::{Watcher, WatchDiff};

// ## Program

pub struct Program {
  pub name: String,
  pub mech: Core,
  watchers: HashMap<u64, Box<Watcher + Send>>,
  pub incoming: Receiver<RunLoopMessage>,
  pub outgoing: Sender<RunLoopMessage>,
}

impl Program {
  pub fn new(name:&str, capacity: usize) -> Program {
    let (outgoing, incoming) = mpsc::channel();
    Program { 
      name: name.to_owned(), 
      mech: Core::new(capacity, 100),
      watchers: HashMap::new(),
      incoming, 
      outgoing 
    }
  }

  pub fn compile_string(&mut self, input: String) {
    let mut lexer = Lexer::new();
    let mut parser = Parser::new();
    let mut compiler = Compiler::new();
    
    lexer.add_string(input.clone());
    let tokens = lexer.get_tokens();
    println!("{:?}", &tokens);
    parser.add_tokens(&mut tokens.clone());
    parser.build_parse_tree();
    println!("{:?}", &parser.clone());
    let ast = compiler.build_syntax_tree(parser.parse_tree);
    println!("{:?}", ast);
    self.mech.runtime.register_blocks(compiler.blocks, &mut self.mech.store);
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
    let watcher_table = Transaction::from_change(Change::NewTable{tag: name, rows: 1, columns});
    self.program.outgoing.send(RunLoopMessage::Transaction(watcher_table));
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
            println!("{} Txn started", name);
            let pre_changes = program.mech.store.len();
            let start_ns = time::precise_time_ns();
            program.mech.process_transaction(&txn);
            // Handle watchers
            for (watcher_name, dirty) in program.mech.watched_index.iter_mut() {
              if *dirty {
                match program.watchers.get_mut(watcher_name) {
                  Some(watcher) => {
                    let mut diff = WatchDiff::new();
                    for i in program.mech.last_transaction .. program.mech.store.change_pointer {
                      let change = &program.mech.store.changes[i];
                      match change {
                        Change::Add{table, row, column, value} => {
                          if table == watcher_name {
                            diff.adds.push((*table, *row, *column, value.as_i64().unwrap()));
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
            let delta_changes = program.mech.store.len() - pre_changes;
            let end_ns = time::precise_time_ns();
            let time = (end_ns - start_ns) as f64;
            let text = match program.mech.store.get_cell(1,1,1) {
              Some(value) => value.as_string().unwrap(),
              None => "",
            };


            println!("{:?}", program.mech);
            //program.compile_string(String::from(text.clone()));
            //println!("{:?}", program.mech.runtime);
            println!("{} Txn took {:0.4?} ms ({:0.0?} cps)", name, time / 1_000_000.0, delta_changes as f64 / (time / 1.0e9));
          },
          (Ok(RunLoopMessage::Stop), _) => {
            paused = true;
          }
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
