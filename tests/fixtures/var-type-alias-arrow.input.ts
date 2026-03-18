interface ConnInfo {
    remote: RemoteInfo;
}

interface RemoteInfo {
    address: string;
}

type GetConnInfo = (host: string) => ConnInfo;

export const getConnInfo: GetConnInfo = (host: string) => ({
    remote: { address: host },
});
