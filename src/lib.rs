// # Mech Server

/*
 Mech Server is a wrapper around the mech runtime. It provides interfaces for 
 controlling the runtime, sending it transactions, and responding to changes.
*/

// ## Prelude
#![feature(extern_prelude)]

extern crate core;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

extern crate clap;
use clap::{Arg, App};

extern crate ws;
use ws::{listen, Message, Sender as WSSender, Handler, CloseCode, Handshake};
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde;
use serde_json::{Error};

extern crate hashbrown;

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

extern crate mech_core;
extern crate mech_syntax;
extern crate mech_program;
extern crate mech_utilities;
use mech_core::{Core, Change, Transaction};
use mech_core::Value;
use mech_core::{TableIndex, Hasher};
use mech_core::{Block, Constraint};
use mech_program::{ProgramRunner, RunLoop};
use mech_utilities::{RunLoopMessage};

use self::client::ClientHandler;

extern crate rand;
use rand::{Rng, thread_rng};

extern crate walkdir;
use walkdir::WalkDir;

// ## Modules

pub mod client;

// ## Static File Server

struct Custom404;

impl AfterMiddleware for Custom404 {
  fn catch(&self, _: &mut Request, _: IronError) -> IronResult<Response> {
      Ok(Response::with((status::NotFound, "File not found...")))
  }
}

pub fn http_server(address: String,) -> std::thread::JoinHandle<()> {
  thread::spawn(move || {
    let mut mount = Mount::new();
    mount.mount("/", Static::new(Path::new("notebook/dist")));
    mount.mount("css", Static::new(Path::new("notebook/css")));
    mount.mount("images", Static::new(Path::new("assets/images")));
    mount.mount("fonts", Static::new(Path::new("notebook/fonts")));
    
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

pub fn websocket_server(address: String, mech_paths: Vec<&str>, persistence_path: &str) {
  println!("{} Websocket Server at {}... ", BrightGreen.paint("Starting:"), address);
  let mut ix = 0;
  match listen(address, |out| {
    ix += 1;
    let client_name = format!("ws_client_{}", ix);
    ClientHandler::new(&client_name, Some(out), Some(&mech_paths), Some(&persistence_path), None)
  }) {
    Ok(_) => {},
    Err(why) => println!("{} Failed to start Websocket Server: {}", BrightRed.paint("Error:"), why),
  };
}
