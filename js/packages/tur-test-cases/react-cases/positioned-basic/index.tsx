import { renderRoot } from "@tur/react-renderer";
import { Stack, Positioned, SizedBox } from "@tur/react";

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
