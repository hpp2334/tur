import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { javascript } from "@codemirror/lang-javascript";
import { oneDark } from "@codemirror/theme-one-dark";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";

interface CodeEditorProps {
    source: string;
    onSave: (source: string) => void;
    readOnly?: boolean;
}

export function CodeEditor({ source, onSave, readOnly }: CodeEditorProps) {
    const containerRef = useRef<HTMLDivElement>(null);
    const viewRef = useRef<EditorView | null>(null);

    // biome-ignore lint/correctness/useExhaustiveDependencies: re-create editor only when source changes
    useEffect(() => {
        if (!containerRef.current) return;

        const state = EditorState.create({
            doc: source,
            extensions: [
                lineNumbers(),
                highlightActiveLine(),
                history(),
                bracketMatching(),
                closeBrackets(),
                javascript({ jsx: true, typescript: true }),
                syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
                oneDark,
                keymap.of([
                    ...closeBracketsKeymap,
                    ...defaultKeymap,
                    ...historyKeymap,
                    {
                        key: "Mod-s",
                        run: (view) => {
                            onSave(view.state.doc.toString());
                            return true;
                        },
                    },
                ]),
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        update.view.dom.classList.add("modified");
                    }
                }),
                EditorView.editable.of(!readOnly),
            ],
        });

        const view = new EditorView({
            state,
            parent: containerRef.current,
        });
        viewRef.current = view;

        return () => {
            view.destroy();
            viewRef.current = null;
        };
    }, [source]);

    useEffect(() => {
        const view = viewRef.current;
        if (!view) return;
        const currentDoc = view.state.doc.toString();
        if (currentDoc !== source) {
            view.dispatch({
                changes: { from: 0, to: currentDoc.length, insert: source },
            });
        }
    }, [source]);

    return <div ref={containerRef} className="code-editor" />;
}
