import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    render,
    SizedBox,
    Text,
} from "@tur/edgy";

const TABS = [{ id: "todolist", label: "TodoList" }];
const activeId = "todolist";

render(() =>
    Row({
        children: [
            Container({
                color: Color.hex("#1a1a2e"),
                width: 200,
                children: [
                    Column({
                        children: TABS.map((tab) =>
                            Container({
                                color: Color.hex(
                                    tab.id === activeId ? "#0f3460" : "#16213e",
                                ),
                                padding: 12,
                                children: [
                                    Text({ text: tab.label, fontSize: 14 }),
                                ],
                            }),
                        ),
                    }),
                ],
            }),
            Container({
                padding: 16,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Center,
                        children: [
                            Text({ text: "Todo List", fontSize: 24 }),
                            SizedBox({ height: 16 }),
                            Column({
                                children: [
                                    Row({
                                        mainAlignment:
                                            MainAxisAlignment.SpaceBetween,
                                        children: [
                                            Text({ text: "Buy milk" }),
                                            Text({ text: "\u2713" }),
                                        ],
                                    }),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ],
    }),
);
