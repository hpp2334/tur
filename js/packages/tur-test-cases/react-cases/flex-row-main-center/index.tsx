import { renderRoot } from "@tur/react-renderer";
import { Row, SizedBox, MainAxisAlignment, CrossAxisAlignment } from "@tur/react";

function FlexRowMainCenter() {
  return (
    <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(FlexRowMainCenter);
