// # Client Handler

// ## Prelude

use ws::{listen, Message, Sender as WSSender, Handler, CloseCode, Handshake};
#[macro_use]
use serde_json::{Error};
use std::fs::{self, File};
use std::io::Read;

use mech_program::{ProgramRunner, RunLoop, RunLoopMessage, ClientMessage};
use mech_core::{Core, Change, Transaction, Value, Index, ErrorType};
use mech_wasm::WebsocketClientMessage;
use term_painter::ToStyle;
use term_painter::Color::*;
use hashbrown::hash_set::HashSet;

use walkdir::WalkDir;

// ## Client Handler

pub struct ClientHandler {
  pub client_name: String,
  out: Option<WSSender>,
  pub running: RunLoop,
  pub input: HashSet<u64>,
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
    if runner.program.errors.len() > 0 {
      let error_notice = format!("Found {} Errors:", &runner.program.errors.len());
      println!("\n{}\n", Red.paint(error_notice));
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
        for (ix,(text,_)) in block.constraints.iter().enumerate() {
          if ix == error.constraint - 1 {
            println!(" {} {}", Red.paint(">"), text);
          } else {
            println!("   {}", BrightBlack.paint(text));
          }
        }
        println!("\n{}", BrightYellow.paint("------------------------------------------------------\n"));
      }
    }
    // register input
    let mut input = HashSet::new();
    for input_register in runner.program.mech.input.iter() {
      input.insert(input_register.table);
    }
    println!("{} Starting run loop.", BrightCyan.paint(format!("[{}]", client_name)));
    let running = runner.run();
    ClientHandler {client_name: client_name.to_owned(), out, running, input}
  }
}

impl Handler for ClientHandler {

  fn on_open(&mut self, handshake: Handshake) -> Result<(),ws::Error> {
    let mut input = Vec::new();
    for input_reg in &self.input {
      input.push(input_reg);
    }
    let json_msg = serde_json::to_string(&input).unwrap();
    match &self.out {
      Some(out) => {
        out.send(Message::Text(json_msg)).unwrap();
      }
      _ => (),
    }
    
    Ok(())
  }

  fn on_request(&mut self, req: &ws::Request) -> Result<ws::Response, ws::Error> {
    //println!("Handler received request:\n{:?}", req);
    ws::Response::from_request(req)
  }

  fn on_message(&mut self, msg: Message) -> Result<(), ws::Error> {
    
    match msg {
      Message::Text(s) => {
        let deserialized: Result<WebsocketClientMessage, Error> = serde_json::from_str(&s);
        match deserialized {
          Ok(WebsocketClientMessage::Transaction(txn)) => {
            //println!("{:?}", txn);
            self.running.send(RunLoopMessage::Transaction(txn));
          },
          _ => (),
        }
      },
      _ => (),
    }

    /*
    match msg {
      Message::Text(s) => {
        let deserialized: Result<WebsocketClientMessage, Error> = serde_json::from_str(&s);
        match deserialized {
          Ok(WebsocketClientMessage::Table(table_id)) => {
            self.running.send(RunLoopMessage::Table(table_id as u64));
          },
          _ => (),
        }
      },
      _ => (),
    }*/

/*
    match self.running.receive() {
      (Ok(ClientMessage::Table(table))) => {
        match table {
          Some(ref table_ref) => {
            match &self.out {
              Some(out) => {
                let table_json = serde_json::to_string(&table_ref.data).unwrap();
                out.send(Message::Text(table_json)).unwrap();
              }
              _ => (),
            }
          },
          None => (),
        }
      },
      _ => (),
    }*/

    Ok(())
    /*
    if let Message::Text(s) = msg {
      let deserialized: Result<WebsocketClientMessage, Error> = serde_json::from_str(&s);
      println!("deserialized = {:?}", deserialized);
      match deserialized {
          Ok(WebsocketClientMessage::Transaction { adds, removes }) => {
            //println!("Txn: {:?} {:?}", adds, removes);
            let txn = from_adds_removes(adds, removes);
            //println!("{:?}", txn);
            self.running.send(RunLoopMessage::Transaction(txn));
          },
          Ok(WebsocketClientMessage::Control(kind)) => {
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
          Ok(WebsocketClientMessage::Code(code)) => {
            self.running.send(RunLoopMessage::Code(code));
          },
          Ok(m) => println!("Unhandled Websocket Message: {:?}", m),
          Err(error) => println!("Error: {:?}", error),
        }
        Ok(())
    } else {
      Ok(())
    }*/
  }

  fn on_close(&mut self, code: CloseCode, reason: &str) {
    println!("WebSocket closing for ({:?}) {}", code, reason);
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