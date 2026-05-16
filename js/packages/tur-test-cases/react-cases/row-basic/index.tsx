import { renderRoot } from "@tur/react-renderer";
import { Row, SizedBox, CrossAxisAlignment } from "@tur/react";

function RowBasic() {
  return (
    <Row crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(RowBasic);
