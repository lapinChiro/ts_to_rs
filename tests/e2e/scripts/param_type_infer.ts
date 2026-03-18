type BinaryOp = (a: number, b: number) => number;
type Predicate = (x: number) => boolean;

const add: BinaryOp = (a, b) => {
    return a + b;
};

const isPositive: Predicate = (x) => {
    return x > 0;
};

function main(): void {
    console.log("add:", add(10, 20));
    console.log("isPositive 5:", isPositive(5));
    console.log("isPositive -3:", isPositive(-3));
}
