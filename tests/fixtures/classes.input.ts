export class User {
    name: string;
    age: number;

    constructor(name: string, age: number) {
        this.name = name;
        this.age = age;
    }

    get_age(): number {
        return this.age;
    }
}

class Point {
    x: number;
    y: number;
}
