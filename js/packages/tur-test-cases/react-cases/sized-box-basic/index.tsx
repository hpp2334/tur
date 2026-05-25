import { SizedBox, Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function SizedBoxLayout() {
    return (
        <SizedBox width={100} height={50}>
            <Text content="Hi" />
        </SizedBox>
    );
}

renderRoot(SizedBoxLayout);
