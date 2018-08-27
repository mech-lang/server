// # Watchers

// ## Prelude

extern crate mech_core;
use mech_core::{Interner, Change};
use mech_core::Value;

// ## Watchers

pub trait Watcher {
  fn get_name(& self) -> String;
  fn get_columns(& self) -> usize;
  fn set_name(&mut self, &str);
  fn process_change(&mut self, change: &Change);
}

pub mod system;