function square(n: number): number {
    return n * n;
}

function sumOfSquares(a: number, b: number): number {
    return square(a) + square(b);
}

export function main(): number {
    const result = sumOfSquares(3, 4);
    return result;
}
