class Counter {
    count: number;

    constructor(initial: number = 0) {
        this.count = initial;
    }

    reset(value: number = 0): void {
        this.count = value;
    }
}

class Config {
    timeout: number;

    constructor(timeout: number = 30) {
        this.timeout = timeout;
    }
}
