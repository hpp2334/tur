import { createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() => Text({ text: "" }));
