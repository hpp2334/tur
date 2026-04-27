import { renderRoot } from "@tur/solidjs-renderer";
import { PointerInteract, Container } from "@tur/solidjs";

function PointerInteractBasic() {
  return <PointerInteract child={<Container width={100} height={50} />} />;
}

renderRoot(PointerInteractBasic);
