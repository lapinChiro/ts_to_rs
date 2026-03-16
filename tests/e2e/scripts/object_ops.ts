interface Person {
    name: string;
    age: number;
}

interface Rectangle {
    width: number;
    height: number;
}

function area(rect: Rectangle): number {
    return rect.width * rect.height;
}

function greet(person: Person): string {
    return "Hello, " + person.name;
}

function main(): void {
    const p: Person = { name: "Alice", age: 30 };
    console.log("name:", p.name);
    console.log("age:", p.age);
    console.log("greeting:", greet(p));

    const r: Rectangle = { width: 5, height: 3 };
    console.log("width:", r.width);
    console.log("height:", r.height);
    console.log("area:", area(r));
}
