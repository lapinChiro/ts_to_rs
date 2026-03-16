fn main() {
    let mut count: f64 = 0.0;
    let mut n: f64 = 1.0;
    while n < 100.0 {
        n = n * 2.0;
        count = count + 1.0;
    }
    println!("{} {}", "doublings to 100:", count);
    let mut items: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let mut found: f64 = -1.0;
    for item in items {
        if item == 3.0 {
            found = item;
            break;
        }
    }
    println!("{} {}", "found:", found);
    let mut sum: f64 = 0.0;
    for i in 0..5 {
        let i = i as f64;
        sum = sum + i;
    }
    println!("{} {}", "sum 0..5:", sum);
}
