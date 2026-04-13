// Multi-overload callable interface: inner fn uses widest signature
interface GetCookie {
    (c: string): string;
    (c: string, key: string): number;
}

const getCookie: GetCookie = (c: string, key?: string): string => {
    return c;
};
