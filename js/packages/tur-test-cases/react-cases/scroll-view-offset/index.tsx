import { Column, CrossAxisAlignment, ScrollView, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ScrollViewOffset() {
    return (
        <ScrollView scrollOffset={100} queryKey={["scroll-view"]}>
            <Column crossAlignment={CrossAxisAlignment.Start}>
                <SizedBox height={200} />
                <SizedBox height={200} />
                <SizedBox height={200} />
            </Column>
        </ScrollView>
    );
}

renderRoot(ScrollViewOffset);
