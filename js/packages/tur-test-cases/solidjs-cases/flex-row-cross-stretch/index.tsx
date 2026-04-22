import { renderRoot } from "@tur/solidjs-renderer";
import { Row, SizedBox } from "@tur/solidjs";

function FlexRowCrossStretch() {
  return (
    <Row>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(FlexRowCrossStretch);
