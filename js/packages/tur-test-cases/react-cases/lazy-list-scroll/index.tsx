import { Color, Container } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React from "react";

const ITEM_HEIGHT = 50;
const ITEM_COUNT = 100;

function LazyListScroll() {
    const children = Array.from({ length: 21 }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static test fixture
        <React.Fragment key={i}>
            <Container
                width={400}
                height={ITEM_HEIGHT}
                color={
                    i % 2 === 0 ? Color.rgb(48, 48, 48) : Color.rgb(26, 26, 26)
                }
            />
        </React.Fragment>
    ));

    // biome-ignore lint/suspicious/noExplicitAny: tur_lazy_list intrinsic has no TS types in test-cases
    const LazyList = "tur_lazy_list" as any;

    return (
        <LazyList
            axis={0}
            itemCount={ITEM_COUNT}
            overscan={0}
            startIndex={0}
            queryKey={["lazy-list-scroll"]}
        >
            {children}
        </LazyList>
    );
}

renderRoot(LazyListScroll);
