#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]
#![deny(unused_mut, unreachable_code)]
use serde::{Serialize, Deserialize};
#[derive(Debug, Clone, PartialEq)]
struct TodoItem {
    title: String,
    done: bool,
}

fn addTodo(items: Vec<TodoItem>) {
    let mut items = items;
    items.push(TodoItem { title: "Learn Rust".to_string(), done: false });
}

fn getTitles(items: Vec<TodoItem>) -> Vec<String> {
    items.iter().cloned().map(|item| item.title).collect::<Vec<_>>()
}

fn getActiveTodos(items: Vec<TodoItem>) -> Vec<TodoItem> {
    items.iter().cloned().filter(|item| !item.done).collect::<Vec<_>>()
}

fn printTodos(items: Vec<TodoItem>) {
    items.iter().cloned().for_each(|item| {
    println!("{}", item.title);
});
}

fn getTodoCount(items: Vec<TodoItem>) -> f64 {
    items.len() as f64
}