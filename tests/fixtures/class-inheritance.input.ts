class Animal {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    speak(): string {
        return `${this.name}`;
    }
}

class Dog extends Animal {
    constructor(name: string) {
        super(name);
    }
    bark(): string {
        return this.speak() + " barks";
    }
}
