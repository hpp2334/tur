import { renderRoot } from "@tur/solidjs-renderer";
import { Column, Container } from "@tur/solidjs";

function VelloColumnBasic() {
  return (
    <Column>
      <Container width={200} height={50} />
      <Container width={200} height={30} />
    </Column>
  );
}

renderRoot(VelloColumnBasic);
