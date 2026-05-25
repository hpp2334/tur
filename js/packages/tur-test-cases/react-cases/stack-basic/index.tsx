import { SizedBox, Stack } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function StackBasic() {
    return (
        <Stack>
            <SizedBox width={100} height={100} />
            <SizedBox width={200} height={200} />
        </Stack>
    );
}

renderRoot(StackBasic);
