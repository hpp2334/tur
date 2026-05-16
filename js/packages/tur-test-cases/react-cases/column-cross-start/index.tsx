import { renderRoot } from "@tur/react-renderer";
import { Column, SizedBox, CrossAxisAlignment } from "@tur/react";

function ColumnCrossStart() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox width={100} height={50} />
    </Column>
  );
}

renderRoot(ColumnCrossStart);
