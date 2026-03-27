#![allow(unused, dead_code, unreachable_code)]
use serde::{Serialize, Deserialize};
fn doSomething() {
    let mut x: f64 = 1.0;
}

fn runCallback(cb: Box<dyn Fn(f64)>) {
    cb(42.0);
}