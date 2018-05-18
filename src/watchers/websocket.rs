extern crate serde_json;
extern crate ws;
use std::sync::mpsc::{self, Sender};
use self::ws::{Sender as WSSender, Message};
use mech::database::{Interner, Transaction, Change};
use mech::indexes::Hasher;
use mech::table::Value;
use super::{Watcher, WatchDiff};
use super::super::program::{RunLoopMessage};

pub struct WebsocketClientWatcher {
    name: String,
    outgoing: Sender<RunLoopMessage>,
    websocket_out: WSSender,
    client_name: String,
}

impl WebsocketClientWatcher {
  pub fn new(outgoing: Sender<RunLoopMessage>, websocket_out: WSSender, client_name: &str) -> WebsocketClientWatcher {
    let text = serde_json::to_string(&json!({"type": "init", "client": client_name})).unwrap();
    websocket_out.send(Message::Text(text)).unwrap();
    let client_websocket = Hasher::hash_str("client/websocket");
    let new_table = Transaction::from_change(Change::NewTable{tag: client_websocket, rows: 100, columns: 4});
    outgoing.send(RunLoopMessage::Transaction(new_table));
    WebsocketClientWatcher { name: "client/websocket".to_string(), outgoing, websocket_out, client_name: client_name.to_owned() }
  }
}

impl Watcher for WebsocketClientWatcher {
  fn get_name(& self) -> String {
    self.name.clone()
  }
  fn set_name(&mut self, name: &str) {
    self.name = name.to_string();
  }
  fn on_diff(&mut self, interner:&mut Interner, diff: WatchDiff) {  
    let adds: Vec<(u64, u64, u64, u64)> = diff.adds.iter().map(|v| {
      match v {
        Change::Add{table, row, column, value} => (*table, *row, *column, value.as_u64()),
        _ => (0, 0 ,0, 0),
      }
    }).collect();
    let removes: Vec<u64> = diff.removes.iter().map(|v| 0).collect();
    let text = serde_json::to_string(&json!({"type": "diff", "adds": adds, "removes": removes, "client": self.client_name})).unwrap();
    self.websocket_out.send(Message::Text(text)).unwrap();
    
  }
}
