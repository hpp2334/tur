import {
    Column,
    Container,
    CrossAxisAlignment,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

interface Message {
    readonly id: number;
    readonly sender: string;
    readonly text: string;
    readonly time: string;
    readonly mine: boolean;
}

const MESSAGES: readonly Message[] = [
    {
        id: 1,
        sender: "Alice",
        text: "Hey, how's the project going?",
        time: "10:02",
        mine: false,
    },
    {
        id: 2,
        sender: "You",
        text: "Pretty good! Just finished the layout engine.",
        time: "10:03",
        mine: true,
    },
    {
        id: 3,
        sender: "Alice",
        text: "Nice! Does it support flexbox?",
        time: "10:04",
        mine: false,
    },
    {
        id: 4,
        sender: "You",
        text: "Yeah, Column and Row with Expanded children",
        time: "10:05",
        mine: true,
    },
    {
        id: 5,
        sender: "Alice",
        text: "What about stacking?",
        time: "10:06",
        mine: false,
    },
    {
        id: 6,
        sender: "You",
        text: "Stack and Positioned, like Flutter",
        time: "10:07",
        mine: true,
    },
    {
        id: 7,
        sender: "Alice",
        text: "That's awesome. Can I see a demo?",
        time: "10:08",
        mine: false,
    },
    {
        id: 8,
        sender: "You",
        text: "Sure, I'll send you the link in a sec",
        time: "10:08",
        mine: true,
    },
    {
        id: 9,
        sender: "Alice",
        text: "Great, thanks!",
        time: "10:09",
        mine: false,
    },
];

function MessageBubble(props: { msg: Message }) {
    const alignment = props.msg.mine
        ? CrossAxisAlignment.End
        : CrossAxisAlignment.Start;

    return (
        <Column crossAlignment={alignment}>
            <Row>
                <Text content={props.msg.sender} fontSize={10} />
                <SizedBox width={8} />
                <Text content={props.msg.time} fontSize={10} />
            </Row>
            <SizedBox height={2} />
            <Container padding={8}>
                <Text content={props.msg.text} fontSize={14} />
            </Container>
        </Column>
    );
}

function Chat() {
    return (
        <Container padding={16}>
            <Column>
                <Row>
                    <Text content="Chat with Alice" fontSize={20} />
                </Row>
                <SizedBox height={4} />
                <Text content="Online" fontSize={12} />
                <SizedBox height={16} />
                <Column>
                    {MESSAGES.map((msg) => (
                        <>
                            <MessageBubble msg={msg} />
                            <SizedBox height={8} />
                        </>
                    ))}
                </Column>
            </Column>
        </Container>
    );
}

renderRoot(Chat);
