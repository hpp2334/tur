import { renderRoot } from "@tur/solidjs-renderer";
import { Column, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function ColumnCrossStart() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={100} height={50} />
    </Column>
  );
}

renderRoot(ColumnCrossStart);
