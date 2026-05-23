import { renderRoot, Color } from "@tur/react-renderer";
import { Stack, Positioned, Container } from "@tur/react";

function FourColorQuadrants() {
  return (
    <Stack>
      <Positioned left={0} top={0}>
        <Container width={100} height={100} color={Color.hex("#ff0000")} />
      </Positioned>
      <Positioned left={100} top={0}>
        <Container width={100} height={100} color={Color.hex("#00ff00")} />
      </Positioned>
      <Positioned left={0} top={100}>
        <Container width={100} height={100} color={Color.hex("#0000ff")} />
      </Positioned>
      <Positioned left={100} top={100}>
        <Container width={100} height={100} color={Color.hex("#ffff00")} />
      </Positioned>
    </Stack>
  );
}

renderRoot(FourColorQuadrants);
