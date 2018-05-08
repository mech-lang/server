// # Mech Server

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
use mech::database::Database;
use mech::table::Value;

// ## Client Handler

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Block { id: String, code: String },
    RemoveBlock { id: String },
    Transaction { adds: Vec<u64>, removes: Vec<u64> },
}

pub struct ClientHandler {
  out: WSSender,
  client_name: String,
}

impl ClientHandler {
  pub fn new(client_name: &str, out: WSSender) -> ClientHandler {
    ClientHandler {out, client_name: client_name.to_owned()}
  }
}

impl Handler for ClientHandler {

    fn on_open(&mut self, handshake: Handshake) -> Result<(),ws::Error> {
      Ok(())
    }

  fn on_request(&mut self, req: &ws::Request) -> Result<ws::Response, ws::Error> {
    println!("Handler received request:\n{:?}", req);
    let message = ClientMessage::Transaction{adds: vec![6, 7, 8], removes: vec![10, 11, 12, 13]};
    let serialized = serde_json::to_string(&message).unwrap();
    self.out.send(serialized);
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
            for add in adds {
              println!("Add: {:?}", add);
            }
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