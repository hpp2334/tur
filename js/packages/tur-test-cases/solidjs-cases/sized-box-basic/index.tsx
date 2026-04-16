import { renderRoot } from "@tur/solidjs-renderer";
import { SizedBox, Text } from "@tur/solidjs";

function SizedBoxLayout() {
  return (
    <SizedBox width={100} height={50}>
      <Text content="Hi" />
    </SizedBox>
  );
}

renderRoot(SizedBoxLayout);
