import { renderRoot } from "@tur/react-renderer";
import { Column, SizedBox, CrossAxisAlignment } from "@tur/react";

function ColumnBasic() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox height={50} />
      <SizedBox height={30} />
    </Column>
  );
}

renderRoot(ColumnBasic);
