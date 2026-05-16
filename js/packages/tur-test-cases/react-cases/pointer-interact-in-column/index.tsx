import { renderRoot } from "@tur/react-renderer";
import {
  Column,
  Container,
  PointerInteract,
  CrossAxisAlignment,
} from "@tur/react";

function PointerInteractInColumn() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <PointerInteract child={<Container width={80} height={40} />} />
      <PointerInteract child={<Container width={60} height={30} />} />
    </Column>
  );
}

renderRoot(PointerInteractInColumn);
