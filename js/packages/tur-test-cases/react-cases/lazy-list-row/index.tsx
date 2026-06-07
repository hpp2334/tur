import { Axis, Color, Container } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React from "react";

const ITEM_WIDTH = 80;
const ITEM_COUNT = 50;

function LazyListRow() {
    const children = Array.from({ length: 21 }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static test fixture
        <React.Fragment key={i}>
            <Container
                width={ITEM_WIDTH}
                height={300}
                color={
                    i % 2 === 0 ? Color.rgb(48, 48, 48) : Color.rgb(26, 26, 26)
                }
            />
        </React.Fragment>
    ));

    return (
        <tur_lazy_list
            axis={Axis.Horizontal}
            itemCount={ITEM_COUNT}
            overscan={0}
            startIndex={0}
            queryKey={["lazy-list-row"]}
        >
            {children}
        </tur_lazy_list>
    );
}

renderRoot(LazyListRow);
