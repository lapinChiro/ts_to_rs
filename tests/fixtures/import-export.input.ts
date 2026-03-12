import { Point } from "./geometry";
import { Config, Logger } from "./utils/config";

export interface User {
    name: string;
    age: number;
}

interface Internal {
    id: number;
}

export function add(a: number, b: number): number {
    return a + b;
}
