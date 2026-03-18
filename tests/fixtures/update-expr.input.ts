function incrementCounter(initial: number): number {
    let count: number = initial;
    count++;
    return count;
}

function decrementCounter(initial: number): number {
    let count: number = initial;
    count--;
    return count;
}

function consumeWhitespace(s: string, start: number): number {
    let startIndex: number = start;
    while (startIndex < s.length) {
        startIndex++;
    }
    return startIndex;
}
