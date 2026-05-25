import { Container, PointerInteract } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function PointerInteractBasic() {
    return <PointerInteract child={<Container width={100} height={50} />} />;
}

renderRoot(PointerInteractBasic);
