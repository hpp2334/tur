import { renderRoot } from "@tur/react-renderer";
import { PointerInteract, Container } from "@tur/react";

function PointerInteractBasic() {
  return <PointerInteract child={<Container width={100} height={50} />} />;
}

renderRoot(PointerInteractBasic);
