// E2E test for mutation detection (I-335, I-255, I-258)

// I-335: user-defined &mut self method
class Counter {
    _count: number;
    constructor(count: number) {
        this._count = count;
    }
    increment(): void {
        this._count += 1;
    }
    get_count(): number {
        return this._count;
    }
}

// I-335: caller of &mut self method should have let mut
function useCounter(counter: Counter): number {
    counter.increment();
    counter.increment();
    return counter.get_count();
}

// I-255: x++ on parameter should generate let mut rebinding
function incrementParam(x: number): number {
    x++;
    return x;
}

// I-258: mutation inside closure capturing parameter
function pushToParam(items: number[]): number {
    const adder = (): void => {
        items.push(99);
    };
    adder();
    return items.length;
}

// I-258: mutation inside control flow on parameter
function conditionalMutate(count: number, shouldIncrement: boolean): number {
    if (shouldIncrement) {
        count += 10;
    }
    return count;
}

function main(): void {
    // I-335
    const c = new Counter(0);
    console.log("counter:", useCounter(c));

    // I-255
    console.log("increment:", incrementParam(5));

    // I-258 closure
    const arr: number[] = [1, 2, 3];
    console.log("push:", pushToParam(arr));

    // I-258 control flow
    console.log("conditional:", conditionalMutate(100, true));
    console.log("conditional:", conditionalMutate(100, false));
}
