import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createTextEditingController,
    Input,
    MainAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useAtomValue } from "jotai/react";
import { atom, getDefaultStore } from "jotai/vanilla";

declare global {
    var __jotaiTestStore: ReturnType<typeof getDefaultStore>;
    var __jotaiInputText: typeof inputTextAtom;
}

const store = getDefaultStore();
const inputTextAtom = atom("");

globalThis.__jotaiTestStore = store;
globalThis.__jotaiInputText = inputTextAtom;

const controller = createTextEditingController({
    onInput: (text: string) => {
        store.set(inputTextAtom, text);
    },
});

function InputJotai() {
    const inputText = useAtomValue(inputTextAtom);
    const canAdd = inputText.trim().length > 0;

    return (
        <Container queryKey={["root"]} padding={16}>
            <Column
                mainAxisSize={MainAxisSize.Min}
                crossAlignment={CrossAxisAlignment.Start}
            >
                <Container queryKey={["input-wrapper"]}>
                    <Input
                        controller={controller}
                        placeholder="Type here..."
                        fontSize={14}
                        width={200}
                        height={30}
                    />
                </Container>
                <SizedBox height={8} />
                <Container
                    queryKey={["button"]}
                    color={canAdd ? Color.hex("#4CAF50") : Color.hex("#CCCCCC")}
                    width={80}
                    height={32}
                    borderRadius={4}
                >
                    <Row
                        mainAlignment={MainAxisAlignment.Center}
                        crossAlignment={CrossAxisAlignment.Center}
                        mainAxisSize={MainAxisSize.Min}
                    >
                        <Text
                            content={canAdd ? "Active" : "Disabled"}
                            fontSize={14}
                            queryKey={["button-text"]}
                        />
                    </Row>
                </Container>
                <SizedBox height={8} />
                <Text
                    content={`text:"${inputText}"`}
                    fontSize={12}
                    queryKey={["debug-text"]}
                />
            </Column>
        </Container>
    );
}

renderRoot(InputJotai);
