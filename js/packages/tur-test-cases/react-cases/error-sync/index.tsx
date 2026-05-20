import { renderRoot } from "@tur/react-renderer";

function App() {
  throw new Error("sync error during render");
}

renderRoot(App);
