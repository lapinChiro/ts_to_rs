fn firstOrNone(items: Vec<String>) -> Option<String> {
    items.get(0).cloned()
}

fn firstOrNoneTernary(cond: bool, items: Vec<String>) -> Option<String> {
    if cond { items.get(0).cloned() } else { None }
}

fn firstOpt(items: Vec<Option<String>>) -> Option<String> {
    items.get(0).cloned().flatten()
}

fn main() {
    let r1: Option<String> = firstOrNone(vec!["a".to_string(), "b".to_string()]);
    let r2: Option<String> = firstOrNone(vec![]);
    let r3: Option<String> = firstOrNone(vec!["solo".to_string()]);
    println!("{} {}", "r1 is undefined:", r1.is_none());
    println!("{} {}", "r2 is undefined:", r2.is_none());
    println!("{} {}", "r3 is undefined:", r3.is_none());
    let arr: Vec<String> = vec!["x".to_string(), "y".to_string()];
    let empty: Vec<String> = vec![];
    let a1: Option<String> = arr.get(0).cloned();
    let a2: Option<String> = empty.get(0).cloned();
    println!("{} {}", "a1 is undefined:", a1.is_none());
    println!("{} {}", "a2 is undefined:", a2.is_none());
    let t1: Option<String> = firstOrNoneTernary(true, vec!["ok".to_string()]);
    let t2: Option<String> = firstOrNoneTernary(true, vec![]);
    let t3: Option<String> = firstOrNoneTernary(false, vec!["skipped".to_string()]);
    println!("{} {}", "t1 is undefined:", t1.is_none());
    println!("{} {}", "t2 is undefined:", t2.is_none());
    println!("{} {}", "t3 is undefined:", t3.is_none());
    let o1: Option<String> = firstOpt(vec![Some("x".to_string())]);
    let o2: Option<String> = firstOpt(vec![None]);
    let o3: Option<String> = firstOpt(vec![]);
    println!("{} {}", "o1 is undefined:", o1.is_none());
    println!("{} {}", "o2 is undefined:", o2.is_none());
    println!("{} {}", "o3 is undefined:", o3.is_none());
}