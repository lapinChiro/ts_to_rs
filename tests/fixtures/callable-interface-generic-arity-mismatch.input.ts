// Generic callable interface with intentional arity mismatch
// Interface expects 2 type params but const uses 1 → conversion error (INV-4)
interface Mapper<T, U> {
    (input: T): U;
}

const mapStr: Mapper<string> = (input: string): string => {
    return input;
};
