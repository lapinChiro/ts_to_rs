// getter only
class Counter {
    _count: number;

    get count(): number {
        return this._count;
    }
}

// setter only
class Writer {
    _name: string;

    set name(value: string) {
        this._name = value;
    }
}

// getter + setter pair
class Person {
    _age: number;

    get age(): number {
        return this._age;
    }

    set age(value: number) {
        this._age = value;
    }
}

// mixed: getter + regular method
class Config {
    _debug: boolean;

    get debug(): boolean {
        return this._debug;
    }

    toggle(): void {
        this._debug = !this._debug;
    }
}
