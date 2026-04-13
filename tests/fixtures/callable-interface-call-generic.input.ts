// Generic callable interface with type substitution at call site
interface Mapper<T, U> {
    (input: T): U;
}

const strToNum: Mapper<string, number> = (input: string): number => {
    return 42;
};

// Call dispatches through substituted signature: (input: String) -> f64
function useMapper(): number {
    return strToNum("hello");
}
