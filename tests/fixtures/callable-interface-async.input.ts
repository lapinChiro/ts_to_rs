// Async callable interface: single overload
interface AsyncFetcher {
    (url: string): Promise<string>;
}

const fetchData: AsyncFetcher = async (url: string): Promise<string> => {
    return url;
};

// Async callable interface: multi-overload with divergent returns
interface AsyncProcessor {
    (data: string): Promise<string>;
    (data: string, flag: boolean): Promise<number>;
}

const processData: AsyncProcessor = async (data: string, flag?: boolean): Promise<any> => {
    if (flag) {
        return 42;
    }
    return data;
};
