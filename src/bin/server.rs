extern crate mech;
extern crate core;
extern crate time;
extern crate rand;
extern crate iron;

use iron::prelude::*;
use iron::status;
use mech::database::Database;

fn main() {
  Iron::new(|_: &mut Request| {
    let mech = Database::new(10000,100);
    println!("{:?}", mech);
    Ok(Response::with((status::Ok, "Hello World!")))
  }).http("localhost:8081").unwrap();
}