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

function main(): void {
    const dog: Dog = new Dog("Rex");
    console.log(dog.speak());
    console.log(dog.name);
}
