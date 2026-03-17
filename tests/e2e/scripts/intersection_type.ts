interface Named {
    name: string;
}

interface Aged {
    age: number;
}

type Person = Named & Aged;

function main(): void {
    const n: Named = { name: "Alice" };
    const a: Aged = { age: 30 };
    console.log("name:", n.name);
    console.log("age:", a.age);

    // I-92: intersection type struct init with String field
    const p: Person = { name: "Bob", age: 25 };
    console.log("person:", p.name, p.age);
}
