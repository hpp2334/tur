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

declare module "*/tur_website.js" {
    export default function init(
        module_or_path: WebAssembly.Module,
    ): Promise<void>;
    // biome-ignore lint/complexity/noStaticOnlyClass: WASM module type declaration
    export class TurWebsiteApp {
        static create_in(id: string): Promise<Record<string, unknown>>;
    }
}
