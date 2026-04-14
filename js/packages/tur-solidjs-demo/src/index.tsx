import { TurRenderer } from "@tur/solidjs-renderer";
import { TodoList } from "./examples/todolist";

declare global {
  var startApp: () => void;
}

globalThis.startApp = (): void => {
  TurRenderer.render(TodoList);
};
