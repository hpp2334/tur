import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    mount,
    Text,
    view,
} from "tur:std";

// Repro: Column(crossAlignment: Stretch) > Container(padding, color, no size)
// > Text. Expected: full-width red strip with white text, visible.
const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            Container({
                color: Color.rgb(0xff, 0x00, 0x00),
                padding: 20,
                queryKey: ["container"],
                children: [
                    Text({
                        text: "I should be visible",
                        color: Color.rgb(0xff, 0xff, 0xff),
                        queryKey: ["text"],
                    }),
                ],
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
