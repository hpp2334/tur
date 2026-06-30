import { Color, Condition, Container, Text, view } from "@tur/edgy";

export default view(() =>
    Condition({
        condition: true,
        queryKey: ["condition-basic"],
        child: () =>
            Container({
                width: 200,
                height: 100,
                color: Color.rgb(48, 48, 48),
                children: [
                    Text({ text: "then-branch", queryKey: ["then-text"] }),
                ],
            }),
        elseChild: () =>
            Container({
                width: 200,
                height: 100,
                color: Color.rgb(80, 26, 26),
                children: [
                    Text({ text: "else-branch", queryKey: ["else-text"] }),
                ],
            }),
    }),
);
