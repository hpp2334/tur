import { renderRoot } from "@tur/solidjs-renderer";
import { Row, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function RowBasic() {
  return (
    <Row crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={50} />
      <SizedBox width={30} />
    </Row>
  );
}

renderRoot(RowBasic);
