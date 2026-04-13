// Generic callable interface: type params are substituted with concrete types
interface Mapper<T, U> {
    (input: T): U;
}

const strToNum: Mapper<string, number> = (input: string): number => {
    return 42;
};
