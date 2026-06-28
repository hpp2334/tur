import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type EdgyElement,
    get,
    MainAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
    Text,
} from "@tur/edgy";
import {
    cancelDelete,
    closeConnect,
    closeNewFolder,
    confirmDelete,
    confirmDelete$,
    nameCtrl,
    newFolderCtrl,
    passCtrl,
    runTest,
    saveServer,
    submitNewFolder,
    testMessage$,
    testStatus$,
    urlCtrl,
    userCtrl,
} from "./state";
import { COLORS } from "./theme";
import { Button, Field, ModalShell } from "./ui";

function dialogCard(width: number, children: EdgyElement[]): EdgyElement {
    return Container({
        width,
        borderRadius: 14,
        padding: 22,
        color: COLORS.panel,
        borderColor: COLORS.border,
        borderWidth: 1,
        shadowColor: COLORS.shadowLg,
        shadowBlur: 30,
        shadowOffset: [0, 12],
        children,
    });
}

// --- Connect / Edit server dialog ----------------------------------------

export function ConnectDialog(): EdgyElement {
    return ModalShell({
        onBackdropClick: closeConnect,
        card: dialogCard(420, [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({
                        text: "WebDAV Server",
                        fontSize: 17,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 16 }),
                    Field({
                        label: "Name",
                        controller: nameCtrl,
                        placeholder: "My server",
                    }),
                    SizedBox({ height: 14 }),
                    Field({
                        label: "Server URL",
                        controller: urlCtrl,
                        placeholder: "https://host:port/path/",
                    }),
                    SizedBox({ height: 14 }),
                    Field({
                        label: "Username",
                        controller: userCtrl,
                        placeholder: "username",
                    }),
                    SizedBox({ height: 14 }),
                    Field({
                        label: "Password",
                        controller: passCtrl,
                        placeholder: "password",
                    }),
                    SizedBox({ height: 12 }),
                    // Authentication method — Basic only (display chip).
                    Row({
                        children: [
                            Text({
                                text: "Authentication",
                                fontSize: 11,
                                color: COLORS.textMuted,
                            }),
                            SizedBox({ width: 10 }),
                            Container({
                                padding: 5,
                                borderRadius: 6,
                                color: COLORS.badgeBg,
                                children: [
                                    Text({
                                        text: "Basic",
                                        fontSize: 11,
                                        color: COLORS.badgeFg,
                                    }),
                                ],
                            }),
                        ],
                    }),
                    SizedBox({ height: 10 }),
                    // Test status line (reactive).
                    Text({
                        text: derive(() => {
                            const s = get(testStatus$);
                            const msg = get(testMessage$);
                            if (s === "testing") return "Testing connection…";
                            if (s === "ok") return `✓ ${msg}`;
                            if (s === "fail") return `✕ ${msg}`;
                            return "";
                        }),
                        fontSize: 12,
                        color: derive(() => {
                            const s = get(testStatus$);
                            if (s === "ok") return COLORS.success;
                            if (s === "fail") return COLORS.danger;
                            return COLORS.textMuted;
                        }),
                    }),
                    SizedBox({ height: 18 }),
                    Row({
                        mainAlignment: MainAxisAlignment.SpaceBetween,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Button({
                                label: "Test Connection",
                                bg: COLORS.subtleButton,
                                fg: COLORS.subtleButtonFg,
                                onClick: runTest,
                            }),
                            Row({
                                mainAlignment: MainAxisAlignment.End,
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    Button({
                                        label: "Cancel",
                                        bg: COLORS.subtleButton,
                                        fg: COLORS.subtleButtonFg,
                                        onClick: closeConnect,
                                    }),
                                    SizedBox({ width: 8 }),
                                    Button({
                                        label: "Save",
                                        bg: COLORS.accent,
                                        fg: COLORS.accentFg,
                                        onClick: saveServer,
                                    }),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ]),
    });
}

// --- New folder dialog ----------------------------------------------------

export function NewFolderDialog(): EdgyElement {
    return ModalShell({
        onBackdropClick: closeNewFolder,
        card: dialogCard(360, [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({
                        text: "New Folder",
                        fontSize: 16,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 14 }),
                    Field({
                        label: "Folder name",
                        controller: newFolderCtrl,
                        placeholder: "untitled",
                    }),
                    SizedBox({ height: 18 }),
                    Row({
                        mainAlignment: MainAxisAlignment.End,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Button({
                                label: "Cancel",
                                bg: COLORS.subtleButton,
                                fg: COLORS.subtleButtonFg,
                                onClick: closeNewFolder,
                            }),
                            SizedBox({ width: 8 }),
                            Button({
                                label: "Create",
                                bg: COLORS.accent,
                                fg: COLORS.accentFg,
                                onClick: submitNewFolder,
                            }),
                        ],
                    }),
                ],
            }),
        ]),
    });
}

// --- Confirm delete dialog -----------------------------------------------

export function ConfirmDeleteDialog(): EdgyElement {
    return ModalShell({
        onBackdropClick: cancelDelete,
        card: dialogCard(360, [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({ text: "Delete?", fontSize: 16, color: COLORS.text }),
                    SizedBox({ height: 8 }),
                    Text({
                        text: derive(() => {
                            const t = get(confirmDelete$);
                            return t
                                ? `"${t.name}" will be permanently deleted.`
                                : "";
                        }),
                        fontSize: 13,
                        color: COLORS.textMuted,
                    }),
                    SizedBox({ height: 18 }),
                    Row({
                        mainAlignment: MainAxisAlignment.End,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Button({
                                label: "Cancel",
                                bg: COLORS.subtleButton,
                                fg: COLORS.subtleButtonFg,
                                onClick: cancelDelete,
                            }),
                            SizedBox({ width: 8 }),
                            Button({
                                label: "Delete",
                                bg: COLORS.danger,
                                fg: COLORS.dangerFg,
                                onClick: confirmDelete,
                            }),
                        ],
                    }),
                ],
            }),
        ]),
    });
}
