#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]
#![deny(unused_mut, unreachable_code)]
use serde::{Serialize, Deserialize};
#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

fn makePoint(x: f64) -> Point {
    Point { x: x, y: 0.0 }
}

#[derive(Debug, Clone, PartialEq)]
struct ConnInfo {
    remote: RemoteInfo,
}

#[derive(Debug, Clone, PartialEq)]
struct RemoteInfo {
    address: String,
}

fn getConnInfo(host: String) -> ConnInfo {
    ConnInfo { remote: RemoteInfo { address: host } }
}