import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    get,
    ImageEdgy,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    type StoreCtx,
    Text,
    type ViewportSize,
    viewportSize$,
} from "builtin:tur/std";
import {
    getIcon,
    openRepo,
    openRepoFromDraft,
    type Repo,
    repoCtrl,
    repoDraft$,
    repoError$,
} from "./state";
import { COLORS } from "./theme";
import { Button, Field } from "./ui";

// Quick-pick suggestions — all under jsDelivr's 50 MB tree cap (verified).
// Clicking one opens the repo directly. Kept short so all three fit on one
// row inside the 440px card (desktop); on mobile they stack vertically.
const SUGGESTIONS: Repo[] = [
    { owner: "facebook", repo: "react", fullName: "facebook/react" },
    {
        owner: "tailwindlabs",
        repo: "tailwindcss",
        fullName: "tailwindlabs/tailwindcss",
    },
    { owner: "vuejs", repo: "core", fullName: "vuejs/core" },
];

// Responsive: cap the card to the available width (viewport minus the outer
// padding from index.ts) so it never overflows on mobile. Desktop viewports
// are wide enough that this stays at 440.
const isMobile = derive(() => get<ViewportSize>(viewportSize$).width < 720);
const cardWidth = derive(() =>
    Math.min(440, get<ViewportSize>(viewportSize$).width - 44),
);

function Suggestion({ repo }: { repo: Repo }): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx: StoreCtx, _ev) => {
                repoCtrl.setSpans([{ content: repo.fullName }]);
                ctx.set(repoDraft$, repo.fullName);
                ctx.set(repoError$, null);
                openRepo(ctx, repo);
            }),
            child: Container({
                padding: 7,
                borderRadius: 8,
                color: COLORS.subtleButton,
                children: [
                    Text({
                        text: repo.fullName,
                        fontSize: 12,
                        color: COLORS.subtleButtonFg,
                    }),
                ],
            }),
        }),
    });
}

export function LandingScreen(): Element {
    return Container({
        alignment: Alignment.Center,
        children: [
            Container({
                width: cardWidth,
                padding: 34,
                borderRadius: 14,
                color: COLORS.panel,
                borderColor: COLORS.border,
                borderWidth: 1,
                shadowColor: COLORS.shadowMd,
                shadowBlur: 40,
                shadowOffset: [0, 8],
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            // Brand mark.
                            Container({
                                width: 40,
                                height: 40,
                                borderRadius: 10,
                                color: COLORS.accentSoft,
                                alignment: Alignment.Center,
                                children: [
                                    ImageEdgy({
                                        resourceId: getIcon("github"),
                                        width: 24,
                                        height: 24,
                                        queryKey: ["gh-mark"],
                                    }),
                                ],
                            }),
                            SizedBox({ height: 18 }),
                            Text({
                                text: "Browse a public repo",
                                fontSize: 18,
                                color: COLORS.text,
                            }),
                            SizedBox({ height: 6 }),
                            Text({
                                text: 'Enter a repo as "owner/name" to explore its files.',
                                fontSize: 13,
                                color: COLORS.textMuted,
                            }),
                            SizedBox({ height: 20 }),
                            Field({
                                label: "Repository",
                                controller: repoCtrl,
                                placeholder: "facebook/react",
                            }),
                            // Inline validation error.
                            Condition({
                                condition: derive(
                                    () => get(repoError$) !== null,
                                ),
                                child: () =>
                                    Column({
                                        mainAxisSize: MainAxisSize.Min,
                                        crossAlignment:
                                            CrossAxisAlignment.Stretch,
                                        children: [
                                            SizedBox({ height: 8 }),
                                            Text({
                                                text: derive(
                                                    () => get(repoError$) ?? "",
                                                ),
                                                fontSize: 12,
                                                color: COLORS.danger,
                                            }),
                                        ],
                                    }),
                                elseChild: () => SizedBox({ height: 0 }),
                            }),
                            SizedBox({ height: 16 }),
                            // Submit row.
                            Row({
                                mainAlignment: MainAxisAlignment.End,
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    Button({
                                        label: "Browse",
                                        bg: COLORS.accent,
                                        fg: COLORS.accentFg,
                                        onClick: openRepoFromDraft,
                                    }),
                                ],
                            }),
                            SizedBox({ height: 22 }),
                            Text({
                                text: "Try one of these",
                                fontSize: 11,
                                color: COLORS.textSubtle,
                            }),
                            SizedBox({ height: 8 }),
                            Condition({
                                condition: isMobile,
                                child: () =>
                                    Column({
                                        mainAxisSize: MainAxisSize.Min,
                                        crossAlignment:
                                            CrossAxisAlignment.Start,
                                        children: SUGGESTIONS.flatMap(
                                            (repo, i) => [
                                                ...(i === 0
                                                    ? []
                                                    : [
                                                          SizedBox({
                                                              height: 6,
                                                          }),
                                                      ]),
                                                Suggestion({ repo }),
                                            ],
                                        ),
                                    }),
                                elseChild: () =>
                                    Row({
                                        mainAxisSize: MainAxisSize.Min,
                                        children: SUGGESTIONS.flatMap(
                                            (repo, i) => [
                                                ...(i === 0
                                                    ? []
                                                    : [SizedBox({ width: 6 })]),
                                                Suggestion({ repo }),
                                            ],
                                        ),
                                    }),
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}
