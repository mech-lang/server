// # Watchers

// ## Prelude

extern crate mech;
use mech::database::Interner;
use mech::table::Value;

// ## Watchers

#[derive(Debug)]
pub struct WatchDiff {
    pub adds: Vec<Vec<Value>>,
    pub removes: Vec<Vec<Value>>,
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
    fn set_name(&mut self, &str);
    fn on_diff(&mut self, interner: &mut Interner, diff: WatchDiff);
}

pub mod system;
pub mod websocket;