import {
    Column,
    Container,
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { Fragment } from "react";

interface SettingItem {
    readonly label: string;
    readonly value: string;
}

interface SettingSection {
    readonly title: string;
    readonly items: readonly SettingItem[];
}

const SECTIONS: readonly SettingSection[] = [
    {
        title: "General",
        items: [
            { label: "Language", value: "English" },
            { label: "Theme", value: "System" },
            { label: "Font Size", value: "Medium" },
        ],
    },
    {
        title: "Notifications",
        items: [
            { label: "Push Notifications", value: "On" },
            { label: "Email Digest", value: "Daily" },
            { label: "Sound", value: "Default" },
        ],
    },
    {
        title: "Privacy",
        items: [
            { label: "Profile Visibility", value: "Friends" },
            { label: "Activity Status", value: "Off" },
            { label: "Data Sharing", value: "Disabled" },
        ],
    },
    {
        title: "Account",
        items: [
            { label: "Email", value: "user@example.com" },
            { label: "Plan", value: "Pro" },
            { label: "Storage Used", value: "4.2 GB" },
        ],
    },
];

function SettingRow(props: { item: SettingItem }) {
    return (
        <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
            <Text content={props.item.label} />
            <Text content={props.item.value} />
        </Row>
    );
}

function Settings() {
    return (
        <Container padding={16}>
            <Column crossAlignment={CrossAxisAlignment.Center}>
                <Text content="Settings" fontSize={28} />
                <SizedBox height={16} />
                {SECTIONS.map((section) => (
                    <Container key={section.title} padding={12}>
                        <Column>
                            <Text content={section.title} fontSize={18} />
                            <SizedBox height={8} />
                            {section.items.map((item) => (
                                <Fragment key={item.label}>
                                    <SettingRow item={item} />
                                    <SizedBox height={4} />
                                </Fragment>
                            ))}
                        </Column>
                    </Container>
                ))}
            </Column>
        </Container>
    );
}

renderRoot(Settings);
