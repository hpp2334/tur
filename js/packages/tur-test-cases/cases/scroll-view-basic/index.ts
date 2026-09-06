import {
    Column,
    CrossAxisAlignment,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    ScrollView({
        queryKey: ["scroll-view"],
        // Stretch content: the ScrollView shrink-wraps to its content (Flutter
        // parity), so stretching the content column is what makes the viewport
        // full-width (400). Content height 600 > 300 → viewport clamps to 300.
        child: Column({
            crossAlignment: CrossAxisAlignment.Stretch,
            children: [
                SizedBox({ height: 200 }),
                SizedBox({ height: 200 }),
                SizedBox({ height: 200 }),
            ],
        }),
    }),
);

export function start() {
    mount(App);
}
