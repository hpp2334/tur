import { renderRoot } from "@tur/solidjs-renderer";
import { Column, SizedBox, MainAxisAlignment, CrossAxisAlignment } from "@tur/solidjs";

function ColumnMainEnd() {
  return (
    <Column
      mainAlignment={MainAxisAlignment.End}
      crossAlignment={CrossAxisAlignment.Start}
    >
      <SizedBox height={50} />
      <SizedBox height={30} />
    </Column>
  );
}

renderRoot(ColumnMainEnd);
