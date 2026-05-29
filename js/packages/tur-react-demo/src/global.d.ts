declare const require: NodeRequire & {
    context(
        directory: string,
        useSubdirectories?: boolean,
        regExp?: RegExp,
        mode?: "sync" | "eager" | "lazy" | "lazy-once",
    ): {
        keys(): string[];
        (id: string): string;
        resolve(id: string): string;
        id: string;
    };
};
