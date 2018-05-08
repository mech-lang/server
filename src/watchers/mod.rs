// # Watchers

// ## Prelude

extern crate mech;
use mech::database::Interner;

// ## Watchers

pub trait Watcher {
    fn get_name(& self) -> String;
    fn set_name(&mut self, &str);
    fn on_diff(&mut self, interner: &mut Interner);
}

pub mod system;