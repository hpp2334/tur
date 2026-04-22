import { renderRoot } from "@tur/solidjs-renderer";
import { Row, Expanded, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function FlexRowExpanded() {
  return (
    <Row crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <Expanded>
        <SizedBox />
      </Expanded>
    </Row>
  );
}

renderRoot(FlexRowExpanded);
