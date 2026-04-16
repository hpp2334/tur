import { renderRoot } from "@tur/solidjs-renderer";
import { Stack, SizedBox } from "@tur/solidjs";

function StackBasic() {
  return (
    <Stack>
      <SizedBox width={100} height={100} />
      <SizedBox width={200} height={200} />
    </Stack>
  );
}

renderRoot(StackBasic);
