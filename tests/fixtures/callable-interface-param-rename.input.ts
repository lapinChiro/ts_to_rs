// Arrow param name differs from interface param name
interface Transformer {
    (input: string): string;
}

const transform: Transformer = (data: string): string => {
    return data;
};
