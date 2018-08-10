// # Mech Server

/*
 Mech Server is a wrapper around the mech runtime. It provides interfaces for 
 controlling the runtime, sending it transactions, and responding to changes.
*/

// ## Prelude

extern crate core;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

extern crate clap;
use clap::{Arg, App};

extern crate ws;
use ws::{listen, Message, Sender as WSSender, Handler, CloseCode, Handshake};
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
use serde_json::{Error};

extern crate time;
use std::time::Duration;

extern crate iron;
extern crate staticfile;
extern crate mount;
use iron::{Iron, Chain, status, Request, Response, IronResult, IronError, AfterMiddleware};
use staticfile::Static;
use mount::Mount;
use std::thread;
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;

extern crate mech;
use mech::{Core, Change, Transaction};
use mech::Value;
use mech::{TableIndex, Hasher};
use mech::{Block, Constraint};
use mech::{Function, Comparator};

extern crate mech_server;
use mech_server::program::{ProgramRunner, RunLoop, RunLoopMessage};
use mech_server::watchers::system::{SystemTimerWatcher};
use mech_server::watchers::websocket::{WebsocketClientWatcher};

extern crate rand;
use rand::{Rng, thread_rng};

extern crate walkdir;
use walkdir::WalkDir;

// ## Client Handler

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Block { id: String, code: String },
    RemoveBlock { id: String },
    Transaction { adds: Vec<(u64, u64, u64, i64)>, removes: Vec<(u64, u64, u64, i64)> },
}

pub struct ClientHandler {
  client_name: String,
  out: WSSender,
  running: RunLoop,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: WSSender, mech_paths: &Vec<&str>) -> ClientHandler {
    let mut runner = ProgramRunner::new(client_name, out.clone(), 1500000);
    let outgoing = runner.program.outgoing.clone();
    runner.attach_watcher(Box::new(SystemTimerWatcher::new(outgoing.clone())));
    runner.attach_watcher(Box::new(WebsocketClientWatcher::new(outgoing.clone(), out.clone(), client_name)));
    // Load programs from supplied directories
    // Read the supplied paths for valid mech files
    let mut paths = Vec::new();
    for path in mech_paths {
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
        println!("{} {}", BrightCyan.paint("Compiling:"), cur_path.replace("\\","/"));
        let mut file = File::open(&cur_path).expect("Unable to open the file");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Unable to read the file");
        runner.load_program(contents);
    }
    let running = runner.run();
    ClientHandler {client_name: client_name.to_owned(), out, running}
  }
}

impl Handler for ClientHandler {

  fn on_open(&mut self, handshake: Handshake) -> Result<(),ws::Error> {
    println!("Connection Opened: {:?}", handshake);
    Ok(())
  }

  fn on_request(&mut self, req: &ws::Request) -> Result<ws::Response, ws::Error> {
    println!("Handler received request:\n{:?}", req);
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
          }
          Ok(m) => {
            //println!("Unhandled Websocket Message: {:?}", m);
          }
          Err(error) => { 
            //println!("Error: {:?}", error);
          }
        }
        Ok(())
    } else {
      Ok(())
    }
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
      txn.adds.push(Change::Add{table, row, column, value: Value::from_i64(value)});
    }
    for (table, row,column, value) in removes {
      txn.removes.push(Change::Remove{table, row, column, value: Value::from_i64(value)});
    }
    txn    
  }

// ## Static File Server

struct Custom404;

impl AfterMiddleware for Custom404 {
  fn catch(&self, _: &mut Request, _: IronError) -> IronResult<Response> {
      Ok(Response::with((status::NotFound, "File not found...")))
  }
}

fn http_server(address: String) -> std::thread::JoinHandle<()> {
  thread::spawn(move || {
    let mut mount = Mount::new();
    mount.mount("/", Static::new(Path::new("assets/index.html")));
    mount.mount("/assets/", Static::new(Path::new("assets/")));
    mount.mount("/dist/", Static::new(Path::new("dist/")));

    let mut chain = Chain::new(mount);
    chain.link_after(Custom404);

    println!("{} HTTP Server at {}... ", BrightGreen.paint("Starting:"), address);
    match Iron::new(chain).http(&address) {
      Ok(_) => {},
      Err(why) => println!("{} Failed to start HTTP Server: {}", BrightRed.paint("Error:"), why),
    };
  })
}

// ## Websocket Connection

fn websocket_server(address: String, mech_paths: Vec<&str>) {
  println!("{} Websocket Server at {}... ", BrightGreen.paint("Starting:"), address);
  let mut ix = 0;
  
  match listen(address, |out| {
    ix += 1;
    let client_name = format!("ws_client_{}", ix);
    ClientHandler::new(&client_name, out, &mech_paths) 
  }) {
    Ok(_) => {},
    Err(why) => println!("{} Failed to start Websocket Server: {}", BrightRed.paint("Error:"), why),
  };
}

// ## Server Entry

fn main() {

  let matches = App::new("Mech Server")
    .version("0.0.1")
    .author("Corey Montella")
    .about("Creates an instance of a Mech server. Default values for options are in parentheses.")
    .arg(Arg::with_name("mech_file_paths")
      .help("The files and folders from which to load .mec files")
      .required(true)
      .multiple(true))
    .arg(Arg::with_name("port")
      .short("p")
      .long("port")
      .value_name("PORT")
      .help("Sets the port for the Mech server (3012)")
      .takes_value(true))
    .arg(Arg::with_name("http-port")
      .short("t")
      .long("http-port")
      .value_name("PORT")
      .help("Sets the port for the HTTP server (8081)")
      .takes_value(true))
    .arg(Arg::with_name("address")
      .short("a")
      .long("address")
      .value_name("ADDRESS")
      .help("Sets the address of the server (127.0.0.1)")
      .takes_value(true))
    .arg(Arg::with_name("persist")
      .short("s")
      .long("persist")
      .value_name("FILE")
      .help("Sets the name for the database to load from and write to")
      .takes_value(true))
    .get_matches();

  let wport = matches.value_of("port").unwrap_or("3012");
  let hport = matches.value_of("http-port").unwrap_or("8081");
  let address = matches.value_of("address").unwrap_or("127.0.0.1");
  let http_address = format!("{}:{}",address,hport);
  let websocket_address = format!("{}:{}",address,wport);
  let mech_paths = matches.values_of("mech_file_paths").map_or(vec![], |files| files.collect());

  http_server(http_address);
  websocket_server(websocket_address, mech_paths);
}