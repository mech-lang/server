extern crate serde_json;
extern crate ws;
use std::sync::mpsc::{self, Sender};
use self::ws::{Sender as WSSender, Message};
use mech_core::{Interner, Transaction, Change};
use mech_core::Hasher;
use mech_core::Value;
use super::{Watcher, WatchDiff};
use super::super::program::{RunLoopMessage};

pub struct WebsocketClientWatcher {
    name: String,
    columns: usize,
    outgoing: Sender<RunLoopMessage>,
    websocket_out: WSSender,
    client_name: String,
}

impl WebsocketClientWatcher {
  pub fn new(outgoing: Sender<RunLoopMessage>, websocket_out: WSSender, client_name: &str) -> WebsocketClientWatcher {
    let text = serde_json::to_string(&json!({"type": "init", "client": client_name})).unwrap();
    websocket_out.send(Message::Text(text)).unwrap();
    WebsocketClientWatcher { name: "client/websocket".to_string(), client_name: client_name.to_owned(), columns: 4, outgoing, websocket_out, }
  }
}

impl Watcher for WebsocketClientWatcher {
  fn get_name(& self) -> String {
    self.name.clone()
  }
  fn set_name(&mut self, name: &str) {
    self.name = name.to_string();
  }
  fn get_columns(& self) -> usize {
    self.columns
  }
  fn on_diff(&mut self, interner:&mut Interner, diff: WatchDiff) {  

    //Change::Set{table, row, column, value} => (String::from("html/export instances"), *table, *row, *column, value.as_u64()),
    let text = serde_json::to_string(&json!({"type": "diff", "adds": diff.adds, "removes": diff.removes, "client": self.client_name})).unwrap();
    self.websocket_out.send(Message::Text(text)).unwrap();
    
  }
}
