import type {
    Axis,
    LazyListController,
    LazyListScrollInfo,
    TurNodeHandle,
} from "@tur/react-renderer";
import { createLazyListController, flushSync } from "@tur/react-renderer";
import type { ReactNode } from "react";
import React, { useState } from "react";

function rangeFrom(start: number, end: number): number[] {
    const items: number[] = [];
    for (let i = start; i <= end; i++) {
        items.push(i);
    }
    return items;
}

interface LazyListInternalProps {
    axis: Axis;
    itemCount: number;
    overscan?: number;
    renderItem: (index: number) => ReactNode;
    queryKey?: string[];
}

function LazyList({
    axis,
    itemCount,
    overscan = 3,
    renderItem,
    queryKey,
}: LazyListInternalProps) {
    const initialEnd = Math.min(itemCount - 1, 20);
    const [startIndex, setStartIndex] = useState(0);
    const [endIndex, setEndIndex] = useState(initialEnd);

    const controller = createLazyListController({
        onVisibleRangeChange: (info: LazyListScrollInfo) => {
            flushSync(() => {
                setStartIndex(info.startIndex);
                setEndIndex(info.endIndex);
            });
        },
    });

    return (
        <tur_lazy_list
            ref={(el: TurNodeHandle) => controller._attach(el, __tur.__ctx)}
            axis={axis}
            itemCount={itemCount}
            overscan={overscan}
            startIndex={startIndex}
            controller={controller}
            queryKey={queryKey}
        >
            {rangeFrom(startIndex, endIndex).map((i) => (
                <React.Fragment key={i}>{renderItem(i)}</React.Fragment>
            ))}
        </tur_lazy_list>
    );
}

export interface LazyColumnProps {
    itemCount: number;
    overscan?: number;
    renderItem: (index: number) => ReactNode;
    queryKey?: string[];
}

export function LazyColumn(props: LazyColumnProps) {
    return (
        <LazyList
            axis={0 as Axis}
            itemCount={props.itemCount}
            overscan={props.overscan}
            renderItem={props.renderItem}
            queryKey={props.queryKey}
        />
    );
}

export interface LazyRowProps {
    itemCount: number;
    overscan?: number;
    renderItem: (index: number) => ReactNode;
    queryKey?: string[];
}

export function LazyRow(props: LazyRowProps) {
    return (
        <LazyList
            axis={1 as Axis}
            itemCount={props.itemCount}
            overscan={props.overscan}
            renderItem={props.renderItem}
            queryKey={props.queryKey}
        />
    );
}
