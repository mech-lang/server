// # Watchers

// ## Prelude

extern crate mech_core;
use mech_core::{Interner, Change};
use mech_core::Value;

// ## Watchers

#[derive(Debug)]
pub struct WatchDiff {
  pub adds: Vec<(u64, u64, u64, Value)>,
  pub removes: Vec<(u64, u64, u64, Value)>,
}

impl WatchDiff {
  pub fn new() -> WatchDiff  {
    WatchDiff {
      adds: Vec::new(),
      removes: Vec::new(),
    }
  }
}

pub trait Watcher {
  fn get_name(& self) -> String;
  fn get_columns(& self) -> usize;
  fn set_name(&mut self, &str);
  fn on_diff(&mut self, interner: &mut Interner, diff: WatchDiff);
}

pub mod system;
pub mod websocket;