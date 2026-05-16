import { renderRoot } from "@tur/react-renderer";
import { SizedBox, Text } from "@tur/react";

function SizedBoxLayout() {
  return (
    <SizedBox width={100} height={50}>
      <Text content="Hi" />
    </SizedBox>
  );
}

renderRoot(SizedBoxLayout);
