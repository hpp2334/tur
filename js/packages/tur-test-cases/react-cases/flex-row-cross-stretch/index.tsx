import { renderRoot } from "@tur/react-renderer";
import { Row, SizedBox } from "@tur/react";

function FlexRowCrossStretch() {
  return (
    <Row>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(FlexRowCrossStretch);
