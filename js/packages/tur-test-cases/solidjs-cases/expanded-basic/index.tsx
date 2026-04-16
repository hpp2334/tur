import { renderRoot } from "@tur/solidjs-renderer";
import { Column, Expanded, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function ExpandedBasic() {
  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <SizedBox height={50} />
      <Expanded>
        <SizedBox />
      </Expanded>
    </Column>
  );
}

renderRoot(ExpandedBasic);
