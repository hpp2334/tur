import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

declare global {
    var __setCursorMidTick: (n: number) => void;
}

const ctrl = createTextEditingController();

function InputCursorMid() {
    const [_tick, setTick] = useState(0);
    globalThis.__setCursorMidTick = setTick;

    return <Input controller={ctrl} fontSize={14} width={200} height={30} />;
}

renderRoot(InputCursorMid);
