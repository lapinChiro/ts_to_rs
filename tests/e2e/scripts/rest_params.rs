fn sum(nums: Vec<f64>) -> f64 {
    let mut total: f64 = 0.0;
    for n in nums {
        total = total + n;
    }
    total
}

fn countArgs(args: Vec<f64>) -> f64 {
    args.len() as f64
}

fn main() {
    println!("{} {}", "sum:", sum(vec![1.0, 2.0, 3.0]));
    println!("{} {}", "sum_empty:", sum(vec![]));
    println!("{} {}", "sum5:", sum(vec![1.0, 2.0, 3.0, 4.0, 5.0]));
    println!("{} {}", "count:", countArgs(vec![10.0, 20.0, 30.0, 40.0]));
    let mut arr: Vec<f64> = vec![10.0, 20.0, 30.0];
    println!("{} {}", "sum_spread:", sum(arr));
}
