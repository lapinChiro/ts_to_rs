interface Point {
    x: number;
    y: number;
}

const makePoint: (x: number) => Point = (x: number) => {
    return { x, y: 0 };
};

interface ConnInfo {
    remote: RemoteInfo;
}

interface RemoteInfo {
    address: string;
}

const getConnInfo: (host: string) => ConnInfo = (host: string) => {
    return { remote: { address: host } };
};
