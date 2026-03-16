function classify(x: number): string {
    if (x > 0) {
        return "positive";
    } else if (x < 0) {
        return "negative";
    } else {
        return "zero";
    }
}

function main(): void {
    console.log("classify 5:", classify(5));
    console.log("classify -3:", classify(-3));
    console.log("classify 0:", classify(0));

    // 三項演算子
    const val: number = 10;
    const result: string = val > 5 ? "big" : "small";
    console.log("ternary:", result);
}
