import { renderRoot } from "@tur/solidjs-renderer";
import { Row, SizedBox, MainAxisAlignment, CrossAxisAlignment } from "@tur/solidjs";

function FlexRowMainEnd() {
  return (
    <Row mainAlignment={MainAxisAlignment.End} crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(FlexRowMainEnd);
