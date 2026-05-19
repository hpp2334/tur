import { renderRoot } from "@tur/react-renderer";
import { Column, SizedBox, CrossAxisAlignment, MainAxisSize } from "@tur/react";

function ColumnNested() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox height={50} />
      <Column crossAlignment={CrossAxisAlignment.Start} mainAxisSize={MainAxisSize.Min}>
        <SizedBox height={30} />
      </Column>
      <SizedBox height={40} />
    </Column>
  );
}

renderRoot(ColumnNested);
