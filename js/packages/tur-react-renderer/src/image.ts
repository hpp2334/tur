import type { ResourceHandle } from "./tur";

export function createImageResource(data: Uint8Array): ResourceHandle {
  return __tur.createImageResource(__tur.__ctx, data);
}
