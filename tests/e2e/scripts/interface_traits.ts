// E2E test for interface → trait conversion
// Tests that class implementing interface with methods produces working code

interface Speaker {
    speak(): string;
}

class Dog implements Speaker {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    speak(): string {
        return this.name + " says woof!";
    }
}

class Cat implements Speaker {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    speak(): string {
        return this.name + " says meow!";
    }
}

interface Describable {
    describe(): string;
}

class Product implements Describable {
    title: string;
    price: number;
    constructor(title: string, price: number) {
        this.title = title;
        this.price = price;
    }
    describe(): string {
        return this.title + " ($" + this.price + ")";
    }
}

function main(): void {
    const dog: Dog = new Dog("Rex");
    console.log(dog.speak());
    console.log(dog.name);

    const cat: Cat = new Cat("Whiskers");
    console.log(cat.speak());
    console.log(cat.name);

    const product: Product = new Product("Widget", 9.99);
    console.log(product.describe());
    console.log(product.title);
}
