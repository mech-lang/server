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
    Transaction { adds: Vec<(u64, u64, u64, i64)>, removes: Vec<(u64, u64, u64, i64)> },
}

pub struct ClientHandler {
  client_name: String,
  out: WSSender,
  running: RunLoop,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: WSSender) -> ClientHandler {
    let mut runner = ProgramRunner::new(client_name, out.clone(), 1500000);
    let outgoing = runner.program.outgoing.clone();
    runner.attach_watcher(Box::new(SystemTimerWatcher::new(outgoing.clone())));
    runner.attach_watcher(Box::new(WebsocketClientWatcher::new(outgoing.clone(), out.clone(), client_name)));
    let program = "# Bouncing Balls

Define the environment
  #html/event/click = [x: 0 y: 0]
  #ball = [x: 15 y: 9 vx: 40 vy: 9]
  #system/timer = [resolution: 15]
  #gravity = 2
  #boundary = 5000

Now update the block positions
  ~ #system/timer.tick
  #ball.x := #ball.x + #ball.vx
  #ball.y := #ball.y + #ball.vy
  #ball.vy := #ball.vy + #gravity

Keep the balls within the y boundary
  ~ #ball.x
  iy = #ball.y > #boundary
  #ball.y[iy] := #boundary
  #ball.vy[iy] := 0 - 1 * #ball.vy * 80 / 100

Keep the balls within the x boundary
  ~ #ball.y
  ix = #ball.x > #boundary
  ixx = #ball.x < 0
  #ball.x[ix] := #boundary
  #ball.x[ixx] := 0
  #ball.vx[ix] := 0 - 1 * #ball.vx * 80 / 100
  #ball.vx[ixx] := 0 - 1 * #ball.vx * 80 / 100
  
Set ball to click
  ~ #html/event/click.x
  #ball += [x: 2 y: 3 vx: 40 vy: 0]";
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