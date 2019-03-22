// # Client Handler

// ## Prelude

use ws::{listen, Message, Sender as WSSender, Handler, CloseCode, Handshake};
#[macro_use]
use serde_json::{Error};
use std::fs::{self, File};
use std::io::Read;

use mech_program::{ProgramRunner, RunLoop, RunLoopMessage};
use mech_core::{Core, Change, Transaction, Value, Index, ErrorType};
use term_painter::ToStyle;
use term_painter::Color::*;

use walkdir::WalkDir;

// ## Client Message

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
  Control { kind: u8 },
  Code { code: String },
  RemoveBlock { id: String },
  Transaction { adds: Vec<(u64, u64, u64, i64)>, removes: Vec<(u64, u64, u64, i64)> },
}

// ## Client Handler

pub struct ClientHandler {
  client_name: String,
  out: Option<WSSender>,
  pub running: RunLoop,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: Option<WSSender>, mech_paths: Option<&Vec<&str>>, persistence_path: Option<&str>) -> ClientHandler {
    let mut runner = ProgramRunner::new(client_name, 1500000);
    let outgoing = runner.program.outgoing.clone();
    // Load programs from supplied directories
    // Read the supplied paths for valid mech files
    let mut paths = Vec::new();
    for path in mech_paths.unwrap_or(&vec![]) {
      let metadata = fs::metadata(path).expect(&format!("Invalid path: {:?}", path));
      if metadata.is_file() {
          paths.push(path.to_string());
      } else if metadata.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
          if entry.file_type().is_file() {
            let ext = entry.path().extension().map(|x| x.to_str().unwrap());
            match ext {
              // Parse .mec and .md files. Add more extensions here to parse those.
              Some("mec") | Some("md") => {
                paths.push(entry.path().canonicalize().unwrap().to_str().unwrap().to_string());
              },
              _ => {}
            }
          }
        }
      }
    }
    // Read each file and parse it
    for cur_path in paths {
        println!("{} {} {}", BrightCyan.paint(format!("[{}]", client_name)), BrightGreen.paint("Compiling:"), cur_path.replace("\\","/"));
        let mut file = File::open(&cur_path).expect("Unable to open the file");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Unable to read the file");
        runner.load_program(contents);
    }
    // Print errors
    println!("\nFound {} Errors:\n", &runner.program.errors.len());
    for error in &runner.program.errors {
      let block = &runner.program.mech.runtime.blocks.get(&(error.block as usize)).unwrap();
      println!("{} {} {} {}\n ", BrightYellow.paint("--"), Yellow.paint("Block"), block.name, BrightYellow.paint("---------------------------------------"));
      match error.error_id {
        ErrorType::DuplicateAlias(alias_id) => {
          let alias = &runner.program.mech.store.names.get(&alias_id).unwrap();
          println!(" Local table {:?} defined more than once.", alias);
        },
        _ => (),
      }
      println!("");
      let (text, constraints) = &block.constraints[error.constraint - 1];
      for constraint in constraints {
        println!(" > {:?}", constraint);
      }
      println!("\n{}", BrightYellow.paint("----------------------------------------------------\n"));
    }
    println!("{}{}{} Starting run loop.", BrightCyan.paint("["), BrightCyan.paint(&runner.name), BrightCyan.paint("]"));
    let running = runner.run();
    ClientHandler {client_name: client_name.to_owned(), out, running}
  }
}

impl Handler for ClientHandler {

  fn on_open(&mut self, handshake: Handshake) -> Result<(),ws::Error> {
    //println!("Connection Opened: {:?}", handshake);
    Ok(())
  }

  fn on_request(&mut self, req: &ws::Request) -> Result<ws::Response, ws::Error> {
    //println!("Handler received request:\n{:?}", req);
    ws::Response::from_request(req)
  }

 fn on_message(&mut self, msg: Message) -> Result<(), ws::Error> {
    //println!("Server got message '{}'. ", msg);
    if let Message::Text(s) = msg {
      let deserialized: Result<ClientMessage, Error> = serde_json::from_str(&s);
      //println!("deserialized = {:?}", deserialized);
      match deserialized {
          Ok(ClientMessage::Transaction { adds, removes }) => {
            //println!("Txn: {:?} {:?}", adds, removes);
            let txn = from_adds_removes(adds, removes);
            //println!("{:?}", txn);
            self.running.send(RunLoopMessage::Transaction(txn));
          },
          Ok(ClientMessage::Control{kind}) => {
            match kind {
              1 => self.running.send(RunLoopMessage::Clear),
              2 => self.running.send(RunLoopMessage::Stop),
              3 => self.running.send(RunLoopMessage::StepBack),
              4 => self.running.send(RunLoopMessage::StepForward),
              5 => self.running.send(RunLoopMessage::Pause),
              6 => self.running.send(RunLoopMessage::Resume),
              _ => Err("Unknown client message"),
            };
          },
          Ok(ClientMessage::Code{code}) => {
            self.running.send(RunLoopMessage::Code(code));
          },
          Ok(m) => println!("Unhandled Websocket Message: {:?}", m),
          Err(error) => println!("Error: {:?}", error),
        }
        Ok(())
    } else {
      Ok(())
    }
  }

  fn on_close(&mut self, code: CloseCode, reason: &str) {
    //println!("WebSocket closing for ({:?}) {}", code, reason);
    //self.router.lock().unwrap().unregister(&self.client_name);
    self.running.close();
  }
}

  pub fn from_adds_removes(adds: Vec<(u64, u64, u64, i64)>, removes: Vec<(u64, u64, u64, i64)>) -> Transaction {
    let mut txn = Transaction::new();
    for (table, row,column, value) in adds {
      //println!("{:?} {:?}", value, Value::from_i64(value.clone()));
      txn.adds.push(Change::Set{table, row: Index::Index(row), column: Index::Index(column), value: Value::from_i64(value)});
    }
    for (table, row,column, value) in removes {
      txn.removes.push(Change::Remove{table, row: Index::Index(row), column: Index::Index(column), value: Value::from_i64(value)});
    }
    txn    
  }