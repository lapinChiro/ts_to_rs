interface HasName {
    name: string;
}

interface HasAge {
    age: number;
}

interface Person {
    name: string;
    age: number;
}

function describePerson(p: Person): string {
    return p.name + " is " + p.age + " years old";
}

function main(): void {
    const person: Person = { name: "Alice", age: 30 };
    console.log("person:", describePerson(person));

    const person2: Person = { name: "Bob", age: 25 };
    console.log("person2:", describePerson(person2));
}
