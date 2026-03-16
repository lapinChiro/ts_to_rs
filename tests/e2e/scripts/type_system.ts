interface Dog {
    kind: string;
    name: string;
    breed: string;
}

interface Cat {
    kind: string;
    name: string;
    indoor: boolean;
}

function describeDog(d: Dog): string {
    return d.name + " is a " + d.breed;
}

function describeCat(c: Cat): string {
    if (c.indoor) {
        return c.name + " is indoor";
    }
    return c.name + " is outdoor";
}

function main(): void {
    const d: Dog = { kind: "dog", name: "Rex", breed: "Labrador" };
    const c: Cat = { kind: "cat", name: "Whiskers", indoor: true };

    console.log("dog:", describeDog(d));
    console.log("cat:", describeCat(c));
}
