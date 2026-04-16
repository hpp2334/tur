import { renderRoot } from "@tur/solidjs-renderer";
import { Column, Expanded, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

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
