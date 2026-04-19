import type { Color } from "./generated/Color";
import type { MainAxisAlignment } from "./generated/MainAxisAlignment";
import type { CrossAxisAlignment } from "./generated/CrossAxisAlignment";

export interface ResolvedStyle {
  direction: "vertical" | "horizontal" | null;
  mainAlignment: MainAxisAlignment | null;
  crossAlignment: CrossAxisAlignment | null;
  gap: number | null;
  fontSize: number | null;
  color: Color | null;
  padding: number | null;
  width: number | null;
  height: number | null;
}
