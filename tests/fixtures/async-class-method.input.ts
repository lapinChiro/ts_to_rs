class DataService {
    async fetchData(url: string): Promise<string> {
        return url;
    }

    processSync(data: string): string {
        return data;
    }
}
