import { renderRoot } from "@tur/react-renderer";
import { Row, Expanded, SizedBox, CrossAxisAlignment } from "@tur/react";

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
