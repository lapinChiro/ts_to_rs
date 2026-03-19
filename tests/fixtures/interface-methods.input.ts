// Interface with methods only → should become a trait
interface Serializable {
    serialize(): string;
    deserialize(data: string): void;
}

// Interface with fields and methods → should become struct + trait + impl
interface Greeter {
    name: string;
    greet(): string;
}

// Interface extends with methods
interface Animal {
    speak(): string;
}

interface Dog extends Animal {
    bark(): string;
}

// Interface extends with fields only
interface Point {
    x: number;
    y: number;
}

interface Point3D extends Point {
    z: number;
}

// Intersection type with methods → should become trait with supertrait
interface Readable {
    read(): string;
}

interface Writable {
    write(data: string): void;
}

type ReadWrite = Readable & Writable;
