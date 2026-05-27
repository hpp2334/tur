import * as React from "react";
import * as TurReact from "@tur/react";
import * as TurReactRenderer from "@tur/react-renderer";
import * as JotaiVanilla from "jotai/vanilla";
import * as JotaiReact from "jotai/react";

(globalThis as Record<string, unknown>).React = React;
(globalThis as Record<string, unknown>).TurReact = TurReact;
(globalThis as Record<string, unknown>).TurReactRenderer = TurReactRenderer;
(globalThis as Record<string, unknown>).JotaiVanilla = JotaiVanilla;
(globalThis as Record<string, unknown>).JotaiReact = JotaiReact;
