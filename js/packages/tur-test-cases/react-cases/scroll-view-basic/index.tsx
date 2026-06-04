import { Column, CrossAxisAlignment, ScrollView, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ScrollViewBasic() {
    return (
        <ScrollView queryKey={["scroll-view"]}>
            <Column crossAlignment={CrossAxisAlignment.Start}>
                <SizedBox height={200} />
                <SizedBox height={200} />
                <SizedBox height={200} />
            </Column>
        </ScrollView>
    );
}

renderRoot(ScrollViewBasic);
