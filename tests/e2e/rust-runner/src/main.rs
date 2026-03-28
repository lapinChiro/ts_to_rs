#[derive(Debug, Clone, PartialEq)]
pub struct _TypeLit0 {
    pub Stringify: f64,
    pub BeforeStream: f64,
    pub Stream: f64,
}

type PhaseValue = f64;

fn describePhase(val: PhaseValue) -> String {
    if val == 1.0 {
        return "stringify".to_string();
    } else {
        if val == 2.0 {
            return "before-stream".to_string();
        } else {
            return "stream".to_string();
        }
    }
}

fn main() {
    println!("{}", describePhase(1.0));
    println!("{}", describePhase(2.0));
    println!("{}", describePhase(3.0));
}