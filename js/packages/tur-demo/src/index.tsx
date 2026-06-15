import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import VConsole from "vconsole";
import { App } from "./App";

new VConsole();

const root = document.getElementById("root");
if (!root) throw new Error("root element not found");
createRoot(root).render(
    <StrictMode>
        <App />
    </StrictMode>,
);
