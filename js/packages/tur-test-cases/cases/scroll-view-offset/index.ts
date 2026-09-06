import {
    Column,
    CrossAxisAlignment,
    createScrollController,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const controller = createScrollController({ initialOffset: 100 });

const App = view(() =>
    ScrollView({
        controller,
        queryKey: ["scroll-view"],
        // Stretch content: the ScrollView shrink-wraps to its content (Flutter
        // parity), so stretching the content column is what makes the viewport
        // full-width.
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
