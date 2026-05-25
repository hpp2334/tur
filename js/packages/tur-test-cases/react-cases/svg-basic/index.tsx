import { Container, Svg } from "@tur/react";
import { createSvgResource, renderRoot } from "@tur/react-renderer";

const svgStr = `<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
  <circle cx="50" cy="50" r="40" fill="steelblue"/>
</svg>`;
const resource = createSvgResource(svgStr);

function SvgBasic() {
    return (
        <Container width={200} height={200}>
            <Svg resource={resource} width={200} height={200} fit={0} />
        </Container>
    );
}

renderRoot(SvgBasic);
