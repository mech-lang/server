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

extern crate term_painter;
use self::term_painter::ToStyle;
use self::term_painter::Color::*;

extern crate mech;
use mech::database::{Database, Change, Transaction};
use mech::table::Value;
use mech::indexes::{TableIndex, Hasher};
use mech::runtime::{Block, Constraint};
use mech::operations::Function;

extern crate mech_server;
use mech_server::program::{ProgramRunner, RunLoop, RunLoopMessage};
use mech_server::watchers::system::{SystemTimerWatcher};
use mech_server::watchers::websocket::{WebsocketClientWatcher};

extern crate rand;
use rand::{Rng, thread_rng};

// ## Client Handler

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Block { id: String, code: String },
    RemoveBlock { id: String },
    Transaction { adds: Vec<(u64, u64, u64, u64)>, removes: Vec<(u64, u64, u64, u64)> },
}

pub struct ClientHandler {
  client_name: String,
  out: WSSender,
  running: RunLoop,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: WSSender) -> ClientHandler {
    let mut runner = ProgramRunner::new(client_name, 1000);
    let outgoing = runner.program.outgoing.clone();
    runner.attach_watcher(Box::new(SystemTimerWatcher::new(outgoing.clone())));
    runner.attach_watcher(Box::new(WebsocketClientWatcher::new(out.clone(), client_name)));

  //------------------------------------------------------
  // Load the bouncing balls program
  //------------------------------------------------------
  let system_timer = Hasher::hash_str("system/timer");
  let ball = Hasher::hash_str("ball");
  runner.program.mech.runtime.register_blocks(vec![position_update()], &mut runner.program.mech.store);
  let mut balls = make_balls(10);
  let mut txn = Transaction::from_changeset(vec![
    Change::NewTable{tag: system_timer, rows: 10, columns: 8}, 
    Change::NewTable{tag: ball, rows: 10, columns: 6}, 
    Change::Add{table: system_timer, row: 1, column: 1, value: Value::from_u64(1000)},
  ]); 
  let txn2 = Transaction::from_changeset(balls);
  outgoing.send(RunLoopMessage::Transaction(txn));
  outgoing.send(RunLoopMessage::Transaction(txn2));
  //------------------------------------------------------






    let running = runner.run();
    ClientHandler {client_name: client_name.to_owned(), out, running}
  }
}

impl Handler for ClientHandler {

    fn on_open(&mut self, handshake: Handshake) -> Result<(),ws::Error> {
      Ok(())
    }

  fn on_request(&mut self, req: &ws::Request) -> Result<ws::Response, ws::Error> {
    println!("Handler received request:\n{:?}", req);
    /*let message = ClientMessage::Transaction{
      adds: vec![(6, 7, 8, 9)], 
      removes: vec![(10, 11, 12, 13)]
    };*/
    //let serialized = serde_json::to_string(&message).unwrap();
    //self.out.send(serialized);
    ws::Response::from_request(req)
  }


 fn on_message(&mut self, msg: Message) -> Result<(), ws::Error> {
    println!("Server got message '{}'. ", msg);
    if let Message::Text(s) = msg {
      let deserialized: Result<ClientMessage, Error> = serde_json::from_str(&s);
      println!("deserialized = {:?}", deserialized);
      match deserialized {
          Ok(ClientMessage::Transaction { adds, removes }) => {
            println!("Txn: {:?} {:?}", adds, removes);
            //self.running.send(RunLoopMessage::Transaction(adds));
            
            for remove in removes {
              println!("Remove: {:?}", remove);
            }
          }
          Ok(m) => {
            println!("Unhandled Websocket Message: {:?}", m);
          }
          Err(error) => { 
            println!("Error: {:?}", error);
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
    //self.running.close();
  }
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

fn websocket_server(address: String) {
  println!("{} Websocket Server at {}... ", BrightGreen.paint("Starting:"), address);
  let mut ix = 0;
  
  match listen(address, |out| {
    ix += 1;
    let client_name = format!("ws_client_{}", ix);
    ClientHandler::new(&client_name, out) 
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
    .arg(Arg::with_name("port")
      .short("p")
      .long("port")
      .value_name("PORT")
      .help("Sets the port for the Eve server (3012)")
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
    .get_matches();

  let wport = matches.value_of("port").unwrap_or("3012");
  let hport = matches.value_of("http-port").unwrap_or("8081");
  let address = matches.value_of("address").unwrap_or("127.0.0.1");
  let http_address = format!("{}:{}",address,hport);
  let websocket_address = format!("{}:{}",address,wport);

  http_server(http_address);
  websocket_server(websocket_address);
}

fn make_balls(n: u64) -> Vec<Change> {
  let mut v = Vec::new();
  for i in 0 .. n + 1 {

    let mut rng = thread_rng();
    let x = rng.gen_range(1, 100);
    let y = rng.gen_range(1, 100);
    let dx = rng.gen_range(1, 10);
    let dy = rng.gen_range(1, 10);
    let ball = Hasher::hash_str("ball");
  
    v.push(Change::Add{table: ball, row: i, column: 1, value: Value::from_u64(x)});
    v.push(Change::Add{table: ball, row: i, column: 2, value: Value::from_u64(y)});
    v.push(Change::Add{table: ball, row: i, column: 3, value: Value::from_u64(dx)});
    v.push(Change::Add{table: ball, row: i, column: 4, value: Value::from_u64(dy)});
    v.push(Change::Add{table: ball, row: i, column: 5, value: Value::from_u64(16)});
    v.push(Change::Add{table: ball, row: i, column: 6, value: Value::from_u64(500)});
  
  }
  v
}

fn position_update() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  let system_timer_change = Hasher::hash_str("system/timer/change");
  block.add_constraint(Constraint::Scan {table: system_timer_change, column: 4, register: 1});
  block.add_constraint(Constraint::Scan {table: ball, column: 1, register: 2});
  block.add_constraint(Constraint::Scan {table: ball, column: 2, register: 3});
  block.add_constraint(Constraint::Scan {table: ball, column: 3, register: 4});
  block.add_constraint(Constraint::Scan {table: ball, column: 4, register: 5});
  block.add_constraint(Constraint::Scan {table: ball, column: 5, register: 6});
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![2, 4], output: 1});
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![3, 5], output: 2});
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![5, 6], output: 3});
  block.add_constraint(Constraint::Insert {table: ball, column: 1, register: 1});
  block.add_constraint(Constraint::Insert {table: ball, column: 2, register: 2});
  block.add_constraint(Constraint::Insert {table: ball, column: 4, register: 3});
  let plan = vec![
    Constraint::Function {operation: Function::Add, parameters: vec![2, 4], output: 1},
    Constraint::Function {operation: Function::Subtract, parameters: vec![3, 5], output: 2},
    Constraint::Function {operation: Function::Add, parameters: vec![5, 6], output: 3},
    Constraint::Insert {table: ball, column: 1, register: 1},
    Constraint::Insert {table: ball, column: 2, register: 2},
    Constraint::Insert {table: ball, column: 4, register: 3},
  ];
  block.plan = plan;
  block
}