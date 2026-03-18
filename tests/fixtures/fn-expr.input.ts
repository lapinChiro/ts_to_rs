function applyTwice(x: number): number {
    const double = function(n: number): number {
        return n * 2;
    };
    return double(double(x));
}

function makeMessage(prefix: string): string {
    const format = function(text: string): string {
        return prefix + ": " + text;
    };
    return format("hello");
}
