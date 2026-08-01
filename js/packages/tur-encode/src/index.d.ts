/**
 * @tur-ng/encode — ambient type declarations for the encoding module.
 *
 * Runtime is a synthetic boa module registered by tur-engine
 * (`builtin_plugins::encode`) under the specifier `"tur:encode"`.
 * Provides UTF-8 text encoding/decoding (boa does not implement the
 * Web Platform `TextDecoder` / `TextEncoder` APIs).
 */

declare module "tur:encode" {
    /**
     * Decode a `Uint8Array` (or `ArrayBuffer`) of UTF-8 bytes into a JS string.
     * Throws `TypeError` on invalid UTF-8.
     */
    export function decodeUtf8(bytes: Uint8Array | ArrayBuffer): string;

    /**
     * Encode a JS string into a `Uint8Array` of UTF-8 bytes.
     */
    export function encodeUtf8(text: string): Uint8Array;
}
