import { Positioned, SizedBox, Stack } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function PositionedBasic() {
    return (
        <Stack>
            <Positioned left={10} top={20}>
                <SizedBox width={50} height={50} />
            </Positioned>
        </Stack>
    );
}

renderRoot(PositionedBasic);
