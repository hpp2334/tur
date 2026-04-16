import { renderRoot } from "@tur/solidjs-renderer";
import { Column, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function ColumnBasic() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox height={50} />
      <SizedBox height={30} />
    </Column>
  );
}

renderRoot(ColumnBasic);
