import { TurRenderer } from "@tur/solidjs-renderer";
import { TodoList } from "./examples/todolist";

globalThis.startApp = () => {
  TurRenderer.render(TodoList);
};
