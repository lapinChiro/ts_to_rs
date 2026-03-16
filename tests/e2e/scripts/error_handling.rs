fn riskyOperation(x: f64) -> f64 {
    let mut _try_result: Result<(), String> = Ok(());
    'try_block: {
        if x < 0.0 {
            _try_result = Err("negative".to_string());
            break 'try_block;
        }
        println!("{} {}", "success:", x);
    }
    if let Err(e) = _try_result {
        println!("caught error");
    }
    x
}

fn safeDivide(a: f64, b: f64) -> f64 {
    let mut _try_result: Result<(), String> = Ok(());
    'try_block: {
        if b == 0.0 {
            _try_result = Err("div by zero".to_string());
            break 'try_block;
        }
        return a / b;
    }
    if let Err(e) = _try_result {
        return 0.0;
    }
    unreachable!();
}

fn main() {
    riskyOperation(5.0);
    riskyOperation(-1.0);
    println!("{} {}", "safe divide 10/2:", safeDivide(10.0, 2.0));
    println!("{} {}", "safe divide 10/0:", safeDivide(10.0, 0.0));
}
