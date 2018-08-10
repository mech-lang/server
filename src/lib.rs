// # Mech Server

// ## Prelude
#![feature(extern_prelude)]

extern crate mech;
extern crate mech_syntax;
extern crate time;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate ws;
extern crate term_painter;
extern crate walkdir;

// ## Modules

pub mod program;
pub mod watchers;
pub mod client;