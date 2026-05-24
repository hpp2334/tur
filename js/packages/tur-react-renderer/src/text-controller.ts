import type { TextEditingController, TextEditingControllerOptions } from "./tur";

export function createTextEditingController(
  options?: TextEditingControllerOptions,
): TextEditingController {
  return __tur.createTextEditingController(__tur.__ctx, options);
}
