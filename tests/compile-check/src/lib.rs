#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]
#![deny(unused_mut, unreachable_code)]
use serde::{Serialize, Deserialize};
mod env;
use env::Bindings;

fn getBindings(b: Bindings) -> Bindings {
    b
}
