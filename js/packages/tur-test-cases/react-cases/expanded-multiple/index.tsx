import { renderRoot } from "@tur/react-renderer";
import { Column, Expanded, SizedBox, CrossAxisAlignment } from "@tur/react";

function ExpandedMultiple() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <Expanded>
        <SizedBox />
      </Expanded>
      <Expanded>
        <SizedBox />
      </Expanded>
    </Column>
  );
}

renderRoot(ExpandedMultiple);
