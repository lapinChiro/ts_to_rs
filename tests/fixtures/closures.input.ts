export const double = (x: number): number => x * 2;

export const greet = (name: string): string => {
  return `Hello, ${name}`;
};

export const getFortyTwo = (): number => 42;

export interface Processor {
  transform: (x: number) => number;
  callback: (a: string, b: number) => boolean;
}
