import { Container } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React from "react";

const ITEM_HEIGHT = 50;
const ITEM_COUNT = 100;

function LazyListScroll() {
    const children = Array.from({ length: 21 }, (_, i) => (
        <React.Fragment key={i}>
            <Container width={400} height={ITEM_HEIGHT} color={i % 2 === 0 ? 0xFF303030 : 0xFF1A1A1A} />
        </React.Fragment>
    ));

    return (
        <tur_lazy_list
            axis={0}
            itemCount={ITEM_COUNT}
            itemExtent={ITEM_HEIGHT}
            overscan={0}
            startIndex={0}
            queryKey={["lazy-list-scroll"]}
        >
            {children}
        </tur_lazy_list>
    );
}

renderRoot(LazyListScroll);
