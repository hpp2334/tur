import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    MainAxisAlignment,
    mount,
    Row,
    SizedBox,
    Text,
    view,
} from "tur:std";

const TABS = [{ id: "todolist", label: "TodoList" }];
const activeId = "todolist";

const App = view(() =>
    Row()
        .children([
            Container()
                .color(Color.hex("#1a1a2e"))
                .width(200)
                .children([
                    Column()
                        .children(
                            TABS.map((tab) =>
                                Container()
                                    .color(
                                        Color.hex(
                                            tab.id === activeId
                                                ? "#0f3460"
                                                : "#16213e",
                                        ),
                                    )
                                    .padding(12)
                                    .children([
                                        Text({ text: tab.label })
                                            .fontSize(14)
                                            .build(),
                                    ])
                                    .build(),
                            ),
                        )
                        .build(),
                ])
                .build(),
            Container()
                .padding(16)
                .children([
                    Column()
                        .crossAlignment(CrossAxisAlignment.Center)
                        .children([
                            Text({ text: "Todo List" }).fontSize(24).build(),
                            SizedBox().height(16).build(),
                            Column()
                                .children([
                                    Row()
                                        .mainAlignment(
                                            MainAxisAlignment.SpaceBetween,
                                        )
                                        .children([
                                            Text({ text: "Buy milk" }).build(),
                                            Text({ text: "\u2713" }).build(),
                                        ])
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                ])
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
