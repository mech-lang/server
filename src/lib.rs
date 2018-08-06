// # Mech Server

// ## Prelude
#![feature(extern_prelude)]

extern crate mech;
extern crate mech_syntax;
extern crate time;
#[macro_use]
extern crate serde_json;

// ## Modules

pub mod program;
pub mod watchers;