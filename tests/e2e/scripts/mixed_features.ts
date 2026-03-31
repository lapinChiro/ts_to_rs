interface Animal {
    name: string;
    sound: string;
}

enum Color {
    Red = "red",
    Green = "green",
    Blue = "blue",
}

function makeAnimal(name: string, sound: string): Animal {
    return { name: name, sound: sound };
}

function greetAnimal(a: Animal): string {
    return "Hello " + a.name + ", you say " + a.sound;
}

function colorName(c: Color): string {
    switch (c) {
        case Color.Red:
            return "RED";
        case Color.Green:
            return "GREEN";
        case Color.Blue:
            return "BLUE";
    }
}

function main(): void {
    // Interface + function
    const cat: Animal = makeAnimal("Cat", "meow");
    console.log(greetAnimal(cat));

    const dog: Animal = { name: "Dog", sound: "woof" };
    console.log(greetAnimal(dog));

    // Enum usage
    console.log("red:", colorName(Color.Red));
    console.log("blue:", colorName(Color.Blue));

    // Array operations
    const nums: number[] = [3, 1, 4, 1, 5];
    const sum: number = nums.reduce((acc: number, x: number): number => acc + x, 0);
    console.log("sum:", sum);

    // Template literal
    const name: string = "World";
    console.log(`hello ${name}`);
}
