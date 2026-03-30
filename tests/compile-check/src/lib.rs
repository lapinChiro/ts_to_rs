#![allow(unused, dead_code, unreachable_code)]
use serde::{Serialize, Deserialize};
mod env;
use env::Bindings;

fn getBindings(b: Bindings) -> Bindings {
    b
}
