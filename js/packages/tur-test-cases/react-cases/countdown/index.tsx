import {
    Column,
    Container,
    CrossAxisAlignment,
    createTextEditingController,
    Input,
    MainAxisAlignment,
    PointerInteract,
    Row,
    Text,
} from "@tur/react";
import type { TextEditingController } from "@tur/react-renderer";
import { renderRoot } from "@tur/react-renderer";
import { useRef, useState } from "react";

const DEFAULT_TIME = 60;

function Countdown() {
    const [remaining, setRemaining] = useState(DEFAULT_TIME);
    const [running, setRunning] = useState(false);
    const [editing, setEditing] = useState(false);
    const [editText, setEditText] = useState("");
    const initialRef = useRef(DEFAULT_TIME);
    const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
    const controllerRef = useRef<TextEditingController | null>(null);

    const start = () => {
        if (running) return;
        setRunning(true);
        timerRef.current = setInterval(() => {
            setRemaining((r) => {
                if (r <= 1) {
                    if (timerRef.current !== null) {
                        clearInterval(timerRef.current);
                        timerRef.current = null;
                    }
                    setRunning(false);
                    return 0;
                }
                return r - 1;
            });
        }, 1000);
    };

    const pause = () => {
        if (!running) return;
        if (timerRef.current !== null) {
            clearInterval(timerRef.current);
            timerRef.current = null;
        }
        setRunning(false);
    };

    const reset = () => {
        pause();
        setRemaining(initialRef.current);
    };

    const openEdit = () => {
        pause();
        setEditText(String(initialRef.current));
        const ctrl = createTextEditingController({
            onInput: (text: string) => {
                setEditText(text);
            },
        });
        controllerRef.current = ctrl;
        setEditing(true);
    };

    const confirmEdit = () => {
        const parsed = parseInt(editText, 10);
        if (!Number.isNaN(parsed) && parsed > 0) {
            initialRef.current = parsed;
            setRemaining(parsed);
        }
        setEditing(false);
        controllerRef.current = null;
    };

    return (
        <Container padding={16} queryKey={["root"]}>
            <Column>
                <Text
                    content={`Countdown: ${remaining}`}
                    queryKey={["display"]}
                />
                <Row
                    mainAlignment={MainAxisAlignment.Start}
                    crossAlignment={CrossAxisAlignment.Start}
                >
                    <PointerInteract
                        onClick={openEdit}
                        child={
                            <Container padding={8} queryKey={["btn-edit"]}>
                                <Text content="Edit" />
                            </Container>
                        }
                    />
                    <PointerInteract
                        onClick={start}
                        child={
                            <Container padding={8} queryKey={["btn-start"]}>
                                <Text content="Start" />
                            </Container>
                        }
                    />
                    <PointerInteract
                        onClick={pause}
                        child={
                            <Container padding={8} queryKey={["btn-pause"]}>
                                <Text content="Pause" />
                            </Container>
                        }
                    />
                    <PointerInteract
                        onClick={reset}
                        child={
                            <Container padding={8} queryKey={["btn-reset"]}>
                                <Text content="Reset" />
                            </Container>
                        }
                    />
                </Row>
                {editing && (
                    <Container padding={16} queryKey={["modal"]}>
                        <Column>
                            <Text content="Set time:" />
                            <Container queryKey={["edit-input"]}>
                                <Input
                                    controller={
                                        controllerRef.current as TextEditingController
                                    }
                                    placeholder="Positive integer"
                                    fontSize={14}
                                    width={200}
                                    height={30}
                                />
                            </Container>
                            <PointerInteract
                                onClick={confirmEdit}
                                child={
                                    <Container
                                        padding={8}
                                        queryKey={["btn-confirm"]}
                                    >
                                        <Text content="Confirm" />
                                    </Container>
                                }
                            />
                        </Column>
                    </Container>
                )}
            </Column>
        </Container>
    );
}

renderRoot(Countdown);
