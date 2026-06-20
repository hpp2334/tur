import {
    Column,
    Container,
    CrossAxisAlignment,
    component,
    Expanded,
    Row,
} from "@tur/edgy";

// Test: Row with crossAlignment=Stretch should give non-flex children the
// row's full height, even when those children have no explicit height prop.
export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    // Sidebar-like: explicit width, no height.
                    Container({
                        width: 100,
                        queryKey: ["sidebar"],
                    }),
                    // Divider-like: narrow explicit width, no height.
                    Container({
                        width: 8,
                        queryKey: ["divider"],
                    }),
                    // Expanded fills remaining.
                    Expanded({
                        child: Container({
                            queryKey: ["expanded-child"],
                        }),
                    }),
                ],
            }),
        ],
    }),
);
