import { renderRoot } from "@tur/react-renderer";
import { Column, Expanded, SizedBox, CrossAxisAlignment } from "@tur/react";

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
