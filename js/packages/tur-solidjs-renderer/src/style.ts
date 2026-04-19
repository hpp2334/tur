import type { Color } from "./generated/Color";
import type { CrossAxisAlignment } from "./generated/CrossAxisAlignment";
import type { FlexDirection } from "./generated/FlexDirection";
import type { MainAxisAlignment } from "./generated/MainAxisAlignment";

export interface ResolvedStyle {
  direction: FlexDirection | null;
  mainAlignment: MainAxisAlignment | null;
  crossAlignment: CrossAxisAlignment | null;
  gap: number | null;
  fontSize: number | null;
  color: Color | null;
  padding: number | null;
  width: number | null;
  height: number | null;
}
