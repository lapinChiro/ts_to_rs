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
}
