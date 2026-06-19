import {
    Column,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    type EdgyComponent,
    type EdgyElement,
    Expanded,
    get,
    Row,
} from "@tur/edgy";
import { layoutFlex, layoutMode$ } from "../state";
import { tokens } from "../theme/tokens";
import { Editor } from "./editor";
import { Sidebar } from "./sidebar";
import { StatusBar } from "./status-bar";
import { Toolbar } from "./toolbar";
import { Viewer } from "./viewer";

function EditorAndViewer(): EdgyElement {
    return Row({
        children: [
            Expanded({
                flex: derive(() => layoutFlex("editor", get(layoutMode$))),
                child: Editor(),
            }),
            Expanded({
                flex: derive(() => layoutFlex("viewer", get(layoutMode$))),
                child: Viewer(),
            }),
        ],
    });
}

export const Shell: EdgyComponent = component(() =>
    Container({
        color: tokens.bg.app,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    Toolbar(),
                    Expanded({
                        child: Row({
                            children: [
                                Sidebar(),
                                Expanded({ child: EditorAndViewer() }),
                            ],
                        }),
                    }),
                    StatusBar(),
                ],
            }),
        ],
    }),
);
