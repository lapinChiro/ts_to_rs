#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]
#![deny(unused_mut, unreachable_code)]
use serde::{Serialize, Deserialize};
fn doSomething() {
    let x: f64 = 1.0;
}

fn runCallback(cb: Box<dyn Fn(f64)>) {
    cb(42.0);
}

type MaybeVoid = Option<String>;

fn maybeReturn(flag: bool) -> Option<String> {
    if flag {
        return Some("hello".to_string());
    }
    None
}

async fn runAsync() {
    println!("done");
}

fn printAll(items: Vec<String>) {
    for item in items {
        println!("{}", item);
    }
}

struct EventHandler {
    onClick: Box<dyn Fn(String)>,
    onClose: Box<dyn Fn()>,
}