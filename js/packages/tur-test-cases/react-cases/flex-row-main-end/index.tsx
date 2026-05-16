import { renderRoot } from "@tur/react-renderer";
import { Row, SizedBox, MainAxisAlignment, CrossAxisAlignment } from "@tur/react";

function FlexRowMainEnd() {
  return (
    <Row mainAlignment={MainAxisAlignment.End} crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(FlexRowMainEnd);
