import {
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    SizedBox,
    Text,
    Color,
    createAnimationController,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

declare const __tur: { __ctx: unknown };

function AnimationBasic() {
    const [ctrl] = useState(() => {
        const c = createAnimationController({
            duration: 600,
            curve: "easeInOut",
        });
        c.setTweens({
            width: { begin: 60, end: 200 },
            height: { begin: 60, end: 200 },
        });
        return c;
    });

    const handlePlay = () => {
        const s = ctrl.status;
        if (s === "forward" || s === "reverse") {
            ctrl.stop();
        } else if (ctrl.value > 0.5) {
            ctrl.reverse();
        } else {
            ctrl.forward();
        }
    };

    return (
        <Expanded>
            <Column
                mainAlignment={MainAxisAlignment.Center}
                crossAlignment={CrossAxisAlignment.Center}
            >
                <Container
                    width={60}
                    height={60}
                    color={Color.rgb(59, 130, 246)}
                    borderRadius={12}
                    ref={(el) => ctrl._attach(el, __tur.__ctx)}
                />
                <SizedBox height={20} />
                <PointerInteract
                    onClick={handlePlay}
                    child={
                        <Container
                            width={120}
                            height={40}
                            color={Color.rgb(34, 197, 94)}
                            borderRadius={8}
                        >
                            <Row
                                mainAlignment={MainAxisAlignment.Center}
                                crossAlignment={CrossAxisAlignment.Center}
                            >
                                <Text
                                    content="Animate"
                                    fontSize={14}
                                    color={Color.rgb(255, 255, 255)}
                                />
                            </Row>
                        </Container>
                    }
                />
            </Column>
        </Expanded>
    );
}

renderRoot(AnimationBasic);
