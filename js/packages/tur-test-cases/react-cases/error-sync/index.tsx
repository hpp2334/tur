import { renderRoot } from "@tur/react-renderer";

function App(): never {
    throw new Error("sync error during render");
}

renderRoot(App);
