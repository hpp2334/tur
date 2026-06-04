import { Column, CrossAxisAlignment, ScrollView, SizedBox, createScrollController } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ScrollViewOffset() {
    const controller = createScrollController({ initialOffset: 100 });

    return (
        <ScrollView controller={controller} queryKey={["scroll-view"]}>
            <Column crossAlignment={CrossAxisAlignment.Start}>
                <SizedBox height={200} />
                <SizedBox height={200} />
                <SizedBox height={200} />
            </Column>
        </ScrollView>
    );
}

renderRoot(ScrollViewOffset);
