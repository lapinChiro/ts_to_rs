// Generic callable interface with default type params (P12.1)
// Interface has 3 type params, 2 with defaults → only 1 required arg
interface Transform<T, U = string, V = number> {
    (input: T): U;
}

// Partial type args: T=string provided, U and V use defaults
const transform: Transform<string> = (input: string): string => {
    return input;
};
