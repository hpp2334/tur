import { Container } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React from "react";

const ITEM_WIDTH = 80;
const ITEM_COUNT = 50;

function LazyListRow() {
    const children = Array.from({ length: 21 }, (_, i) => (
        <React.Fragment key={i}>
            <Container width={ITEM_WIDTH} height={300} color={i % 2 === 0 ? 0xFF303030 : 0xFF1A1A1A} />
        </React.Fragment>
    ));

    return (
        <tur_lazy_list
            axis={1}
            itemCount={ITEM_COUNT}
            itemExtent={ITEM_WIDTH}
            overscan={0}
            startIndex={0}
            queryKey={["lazy-list-row"]}
        >
            {children}
        </tur_lazy_list>
    );
}

renderRoot(LazyListRow);
