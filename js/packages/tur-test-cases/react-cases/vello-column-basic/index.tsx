import { Column, Container } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function VelloColumnBasic() {
    return (
        <Column>
            <Container width={200} height={50} />
            <Container width={200} height={30} />
        </Column>
    );
}

renderRoot(VelloColumnBasic);
