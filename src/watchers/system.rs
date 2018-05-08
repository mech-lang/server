extern crate time;
use super::Watcher;
use super::super::program::{RunLoopMessage};
use mech::database::Interner;
use std::sync::mpsc::{self, Sender};
use std::thread::{self};
use std::collections::{HashMap};
use std::collections::hash_map::{Entry};

pub struct SystemTimerWatcher {
    name: String,
    outgoing: Sender<RunLoopMessage>,
    timers: HashMap<u64, (usize, Sender<()>)>
}

impl SystemTimerWatcher {
    pub fn new(outgoing: Sender<RunLoopMessage>) -> SystemTimerWatcher {
        SystemTimerWatcher { name: "system/timer".to_string(), outgoing, timers: HashMap::new() }
    }
}

impl Watcher for SystemTimerWatcher {
    fn get_name(& self) -> String {
        self.name.clone()
    }
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn on_diff(&mut self, interner: &mut Interner) {
    }
}
