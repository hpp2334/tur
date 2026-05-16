import { renderRoot } from "@tur/react-renderer";
import { Column, SizedBox, MainAxisAlignment, CrossAxisAlignment } from "@tur/react";

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
