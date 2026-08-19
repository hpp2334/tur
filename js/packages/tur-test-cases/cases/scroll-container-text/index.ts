import {
    Column,
    Container,
    createStore,
    ScrollView,
    Text,
    view,
} from "tur:std";

export const store = createStore();

// Repro #6: Container(no explicit size, padding:48) sized from a Text child,
// wrapped in ScrollView(axis: Vertical) > Column. Expected: the container is
// laid out at its intrinsic size inside the scrollable column.
export default view(() =>
    ScrollView({
        axis: "vertical",
        child: Column({
            children: [
                Container({
                    padding: 48,
                    queryKey: ["container"],
                    children: [
                        Text({
                            text: "Empty State",
                            fontSize: 24,
                            queryKey: ["text"],
                        }),
                    ],
                }),
            ],
        }),
    }),
);
