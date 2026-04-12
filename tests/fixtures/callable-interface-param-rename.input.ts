// Arrow param name differs from interface param name
interface StringMapper {
    (input: string): string;
}

const mapString: StringMapper = (data: string): string => {
    return data;
};
