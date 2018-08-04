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

// ## Client Handler

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Block { id: String, code: String },
    RemoveBlock { id: String },
    Transaction { adds: Vec<(u64, u64, u64, String)>, removes: Vec<(u64, u64, u64, String)> },
}

pub struct ClientHandler {
  client_name: String,
  out: WSSender,
  running: RunLoop,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: WSSender) -> ClientHandler {
    let mut runner = ProgramRunner::new(client_name, 1500000);
    let outgoing = runner.program.outgoing.clone();
    runner.attach_watcher(Box::new(SystemTimerWatcher::new(outgoing.clone())));
    runner.attach_watcher(Box::new(WebsocketClientWatcher::new(outgoing.clone(), out.clone(), client_name)));
    let program = "# Bouncing Balls
Define the environment
  #ball = [x: 15 y: 9 vx: 18 vy: 9]
  #system/timer = [resolution: 15]
  #gravity = 10
  
Now update the block positions
  ~ #system/timer.tick
  #ball.x := #ball.x + 1
  #ball.y := #ball.y + 1
  #ball.vy := #ball.vy + #gravity";
    runner.load_program(String::from(program));
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
    println!("Server got message '{}'. ", msg);
    if let Message::Text(s) = msg {
      let deserialized: Result<ClientMessage, Error> = serde_json::from_str(&s);
      println!("deserialized = {:?}", deserialized);
      match deserialized {
          Ok(ClientMessage::Transaction { adds, removes }) => {
            println!("Txn: {:?} {:?}", adds, removes);
            let txn = from_adds_removes(adds, removes);
            println!("{:?}", txn);
            self.running.send(RunLoopMessage::Transaction(txn));
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
    self.running.close();
  }
}

  pub fn from_adds_removes(adds: Vec<(u64, u64, u64, String)>, removes: Vec<(u64, u64, u64, String)>) -> Transaction {
    let mut txn = Transaction::new();
    for (table, row,column, value) in adds {
      println!("{:?} {:?}", value, Value::from_string(value.clone()));
      txn.adds.push(Change::Add{table, row, column, value: Value::from_string(value)});
    }
    for (table, row,column, value) in removes {
      txn.removes.push(Change::Remove{table, row, column, value: Value::from_string(value)});
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
    .get_matches();

  let wport = matches.value_of("port").unwrap_or("3012");
  let hport = matches.value_of("http-port").unwrap_or("8081");
  let address = matches.value_of("address").unwrap_or("127.0.0.1");
  let http_address = format!("{}:{}",address,hport);
  let websocket_address = format!("{}:{}",address,wport);

  http_server(http_address);
  websocket_server(websocket_address);
}

/*
fn make_balls(n: u64) -> Vec<Change> {
  let mut v = Vec::new();
  for i in 0 .. n + 1 {

    let mut rng = thread_rng();
    let x = rng.gen_range(1, 500);
    let y = rng.gen_range(1, 500);
    let dx = rng.gen_range(1, 100);
    let dy = rng.gen_range(1, 100);
    let ball = Hasher::hash_str("ball");
  
    v.push(Change::Add{table: ball, row: i, column: 1, value: Value::from_u64(x)});
    v.push(Change::Add{table: ball, row: i, column: 2, value: Value::from_u64(y)});
    v.push(Change::Add{table: ball, row: i, column: 3, value: Value::from_u64(dx)});
    v.push(Change::Add{table: ball, row: i, column: 4, value: Value::from_u64(0)});
  
  }
  v
}



/*
block
  [#ball x y vx vy]
  x := x + vx
  y := y + vy
  vy := vy + 9.8 m/s
end
*/
fn position_update() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  let system_timer_change = Hasher::hash_str("system/timer");
  block.add_constraint(Constraint::ChangeScan {table: system_timer_change, column: 4, input: 1});
  block.add_constraint(Constraint::Scan {table: ball, column: 1, input: 2});
  block.add_constraint(Constraint::Scan {table: ball, column: 2, input: 3});
  block.add_constraint(Constraint::Scan {table: ball, column: 3, input: 4});
  block.add_constraint(Constraint::Scan {table: ball, column: 4, input: 5});  
  block.add_constraint(Constraint::Identity {source: 2, sink: 1});
  block.add_constraint(Constraint::Identity {source: 4, sink: 2});
  block.add_constraint(Constraint::Identity {source: 3, sink: 3});
  block.add_constraint(Constraint::Identity {source: 5, sink: 4});
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![1, 2], output: 5}); 
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![3, 4], output: 6});
  block.add_constraint(Constraint::Constant {value: 1, input: 7});
  block.add_constraint(Constraint::Function {operation: Function::Add, parameters: vec![4, 7], output: 8});
  block.add_constraint(Constraint::Insert {output: 5, table: ball, column: 1});
  block.add_constraint(Constraint::Insert {output: 6, table: ball, column: 2});
  block.add_constraint(Constraint::Insert {output: 7, table: ball, column: 4});
  let plan = vec![
    Constraint::ChangeScan {table: system_timer_change, column: 4, input: 1},
    Constraint::Identity {source: 2, sink: 1},
    Constraint::Identity {source: 4, sink: 2},
    Constraint::Identity {source: 3, sink: 3},
    Constraint::Identity {source: 5, sink: 4},
    Constraint::Constant {value: 2, input: 7},
    Constraint::Function {operation: Function::Add, parameters: vec![1, 2], output: 5},
    Constraint::Function {operation: Function::Add, parameters: vec![3, 4], output: 6},
    Constraint::Function {operation: Function::Add, parameters: vec![4, 7], output: 8},
    Constraint::Insert {output: 5, table: ball, column: 1},
    Constraint::Insert {output: 6, table: ball, column: 2},
    Constraint::Insert {output: 8, table: ball, column: 4},
  ];
  block.plan = plan;
  block
}


/*
block
  [#ball x y]
  ws = #[client/websocket]
  ws.send += x
  ws.send += y
end
*/
fn export_ball() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  let websocket = Hasher::hash_str("client/websocket");
  block.add_constraint(Constraint::Scan {table: ball, column: 1, input: 1});
  block.add_constraint(Constraint::Scan {table: ball, column: 2, input: 2});
  block.add_constraint(Constraint::Identity {source: 1, sink: 1});
  block.add_constraint(Constraint::Identity {source: 2, sink: 2});
  block.add_constraint(Constraint::Insert {output: 1, table: websocket, column: 1});
  block.add_constraint(Constraint::Insert {output: 2, table: websocket, column: 2});
  let plan = vec![
    Constraint::Identity {source: 1, sink: 1},
    Constraint::Identity {source: 2, sink: 2},
    Constraint::Insert {output: 1, table: websocket, column: 1 },
    Constraint::Insert {output: 2, table: websocket, column: 2 },
  ];
  block.plan = plan;
  block
}


/*
block
  click = [#html/event/click/change]
  [#ball x y]
  x := click.x
  y := click.y 
end
*/
fn reset_balls() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  let click = Hasher::hash_str("html/event/click");
  block.add_constraint(Constraint::ChangeScan {table: click, column: 1, input: 1});
  block.add_constraint(Constraint::ChangeScan {table: click, column: 2, input: 2});
  block.add_constraint(Constraint::Identity {source: 1, sink: 1});
  block.add_constraint(Constraint::Identity {source: 2, sink: 2});
  block.add_constraint(Constraint::Constant {value: 10, input: 3});
  block.add_constraint(Constraint::Function {operation: Function::Multiply, parameters: vec![1, 3], output: 4});
  block.add_constraint(Constraint::Function {operation: Function::Multiply, parameters: vec![2, 3], output: 5});
  block.add_constraint(Constraint::Set {output: 1, table: ball, column: 1});
  block.add_constraint(Constraint::Set {output: 2, table: ball, column: 2});
  let plan = vec![
    Constraint::ChangeScan {table: click, column: 1, input: 1},
    Constraint::ChangeScan {table: click, column: 2, input: 2},
    Constraint::Identity {source: 1, sink: 1},
    Constraint::Identity {source: 2, sink: 2},
    Constraint::Constant {value: 10, input: 3},
    Constraint::Function {operation: Function::Multiply, parameters: vec![1, 3], output: 4},
    Constraint::Function {operation: Function::Multiply, parameters: vec![2, 3], output: 5},
    Constraint::Set {output: 4, table: ball, column: 1},
    Constraint::Set {output: 5, table: ball, column: 2},
  ];
  block.plan = plan;
  block
}


/*
block
  [#ball y > 5000, vy]
  vy := -.9 * vy
end
*/
fn boundary_check() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  block.add_constraint(Constraint::ChangeScan {table: ball, column: 2, input: 1});
  block.add_constraint(Constraint::Identity {source: 1, sink: 1});  
  block.add_constraint(Constraint::Constant {value: 5000, input: 2});
  block.add_constraint(Constraint::Filter {comparator: Comparator::GreaterThan, lhs: 1, rhs: 2, intermediate: 3});
  block.add_constraint(Constraint::Scan {table: ball, column: 4, input: 2});
  block.add_constraint(Constraint::Identity {source: 2, sink: 4});     
  block.add_constraint(Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5});
  block.add_constraint(Constraint::Constant {value: -9, input: 6});
  block.add_constraint(Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7});
  block.add_constraint(Constraint::Constant {value: 10, input: 8});
  block.add_constraint(Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9});
  block.add_constraint(Constraint::Insert {output: 9, table: ball, column: 4});
  let plan = vec![
    Constraint::ChangeScan {table: ball, column: 2, input: 1},
    Constraint::Identity {source: 1, sink: 1},
    Constraint::Constant {value: 5000, input: 2},
    Constraint::Filter {comparator: Comparator::GreaterThan, lhs: 1, rhs: 2, intermediate: 3},
    Constraint::Identity {source: 2, sink: 4},
    Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5},
    Constraint::Constant {value: -9, input: 6},
    Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7},
    Constraint::Constant {value: 10, input: 8},
    Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9},
    Constraint::Insert {output: 9, table: ball, column: 4}
  ];
  block.plan = plan;
  block
}


/*
block
  [#ball x > 5000, vx]
  vx := -.9 * vx
end
*/
fn boundary_check2() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  block.add_constraint(Constraint::ChangeScan {table: ball, column: 2, input: 1});
  block.add_constraint(Constraint::Identity {source: 1, sink: 1});  
  block.add_constraint(Constraint::Constant {value: 5000, input: 2});
  block.add_constraint(Constraint::Filter {comparator: Comparator::GreaterThan, lhs: 1, rhs: 2, intermediate: 3});
  block.add_constraint(Constraint::Scan {table: ball, column: 4, input: 2});
  block.add_constraint(Constraint::Identity {source: 2, sink: 4});     
  block.add_constraint(Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5});
  block.add_constraint(Constraint::Constant {value: -9, input: 6});
  block.add_constraint(Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7});
  block.add_constraint(Constraint::Constant {value: 10, input: 8});
  block.add_constraint(Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9});
  block.add_constraint(Constraint::Insert {output: 9, table: ball, column: 4});
  let plan = vec![
    Constraint::ChangeScan {table: ball, column: 2, input: 1},
    Constraint::Identity {source: 1, sink: 1},
    Constraint::Constant {value: 5000, input: 2},
    Constraint::Filter {comparator: Comparator::GreaterThan, lhs: 1, rhs: 2, intermediate: 3},
    Constraint::Identity {source: 2, sink: 4},
    Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5},
    Constraint::Constant {value: -9, input: 6},
    Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7},
    Constraint::Constant {value: 10, input: 8},
    Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9},
    Constraint::Insert {output: 9, table: ball, column: 3}
  ];
  block.plan = plan;
  block
}

/*
block
  [#ball x, vx]
  x < 0
  vx := -.9 * vx
end
*/
fn boundary_check3() -> Block {
  let mut block = Block::new();
  let ball = Hasher::hash_str("ball");
  block.add_constraint(Constraint::ChangeScan {table: ball, column: 2, input: 1});
  block.add_constraint(Constraint::Identity {source: 1, sink: 1});  
  block.add_constraint(Constraint::Constant {value: 0, input: 2});
  block.add_constraint(Constraint::Filter {comparator: Comparator::LessThan, lhs: 1, rhs: 2, intermediate: 3});
  block.add_constraint(Constraint::Scan {table: ball, column: 4, input: 2});
  block.add_constraint(Constraint::Identity {source: 2, sink: 4});     
  block.add_constraint(Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5});
  block.add_constraint(Constraint::Constant {value: -9, input: 6});
  block.add_constraint(Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7});
  block.add_constraint(Constraint::Constant {value: 10, input: 8});
  block.add_constraint(Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9});
  block.add_constraint(Constraint::Insert {output: 9, table: ball, column: 4});
  let plan = vec![
    Constraint::ChangeScan {table: ball, column: 2, input: 1},
    Constraint::Identity {source: 1, sink: 1},
    Constraint::Constant {value: 0, input: 2},
    Constraint::Filter {comparator: Comparator::LessThan, lhs: 1, rhs: 2, intermediate: 3},
    Constraint::Identity {source: 2, sink: 4},
    Constraint::IndexMask{ source: 4, truth: 3, intermediate: 5},
    Constraint::Constant {value: -9, input: 6},
    Constraint::Function {operation: Function::Multiply, parameters: vec![5, 6], output: 7},
    Constraint::Constant {value: 10, input: 8},
    Constraint::Function {operation: Function::Divide, parameters: vec![7, 8], output: 9},
    Constraint::Insert {output: 9, table: ball, column: 3}
  ];
  block.plan = plan;
  block
}
*/