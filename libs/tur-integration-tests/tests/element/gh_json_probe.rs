//! The github-viewer crumb bug — the case's crumb read "react/react" where
//! boa (and GitHub) say "facebook/react"; the audit confirmed the rest of
//! the metadata (description, stars) parsed correctly from the same
//! payload. These tests run the case's JSON scanner VERBATIM (copied — the
//! case is the source of truth) against a realistic repo meta payload and
//! pin what lands on `full_name`.

use tur_integration_tests::TurTestApp;

const PROBE_RUT: &str = r#"
use tur::{ ctx_bridge, mount };
use tur_kit::{ Column, MutationCtx, Readable, Text, source_str };

// ASCII codepoints the scanner compares against (typed so the literals
// land on `u32`).
let C_QUOTE: u32 = 34;
let C_COMMA: u32 = 44;
let C_COLON: u32 = 58;
let C_PIPE: u32 = 124;
let C_SPACE: u32 = 32;
let C_SLASH: u32 = 47;
let C_BACKSLASH: u32 = 92;
let C_LBRACE: u32 = 123;
let C_RBRACE: u32 = 125;
let C_LBRACKET: u32 = 91;
let C_RBRACKET: u32 = 93;

entry fn start() -> u64 {
    let full: Readable<str> = source_str("");
    let desc: Readable<str> = source_str("");
    let col = Column()
        .child(Text().text_bound(full).query_key("ghj-full").build())
        .child(Text().text_bound(desc).query_key("ghj-desc").build())
        .build();
    mount(col);
    return full.atom_id();
}

// Run the scanner over META (the payload rides the f64 arg as a selector
// into the case's payloads) and land the fields on the bound strings.
entry fn probe(atom: u64, _b: f64) {
    let meta = "{\"id\":10270450,\"node_id\":\"MDEwOlJlcG9zaXRvcnkxMDI3MDQ1MA==\",\"name\":\"react\",\"full_name\":\"facebook/react\",\"private\":false,\"owner\":{\"login\":\"facebook\",\"id\":69631,\"node_id\":\"MDEyOk9yZ2FuaXphdGlvbjY5NjMx\",\"avatar_url\":\"https://avatars.githubusercontent.com/u/69631?v=4\",\"gravatar_id\":\"\",\"url\":\"https://api.github.com/users/facebook\",\"html_url\":\"https://github.com/facebook\",\"followers_url\":\"https://api.github.com/users/facebook/followers\",\"type\":\"Organization\",\"site_admin\":false},\"html_url\":\"https://github.com/facebook/react\",\"description\":\"The library for web and native user interfaces.\",\"fork\":false,\"url\":\"https://api.github.com/repos/facebook/react\",\"stargazers_count\":237000,\"watchers_count\":237000,\"language\":\"JavaScript\",\"open_issues_count\":995,\"license\":{\"key\":\"mit\",\"name\":\"MIT License\",\"spdx_id\":\"MIT\"},\"forks\":48500,\"default_branch\":\"main\"}";
    // The desc atom mints right after full — the probe addresses the pair.
    let write = MutationCtx.over(ctx_bridge());
    write.set_str(Readable<str>.of(ctx_bridge(), atom), json_get_str(meta, "full_name"));
    write.set_str(Readable<str>.of(ctx_bridge(), atom + 1), json_get_str(meta, "description"));
}

fn skip_ws(t: str, i: i32) -> i32 {
    let n = t.len();
    let mut j = i;
    while (j < n && ws(t.code_at(j))) {
        j = j + 1;
    }
    return j;
}

fn ws(c: u32) -> bool {
    return c == C_SPACE || c == 9 || c == 10 || c == 13;
}

fn str_end(t: str, i: i32) -> i32 {
    let n = t.len();
    let mut j = i + 1;
    while (j < n) {
        let c = t.code_at(j);
        if (c == C_BACKSLASH) {
            j = j + 2;
            continue;
        }
        if (c == C_QUOTE) {
            return j + 1;
        }
        j = j + 1;
    }
    return n;
}

fn str_unquote(t: str, i: i32) -> str {
    let end = str_end(t, i);
    let raw = t.slice(i + 1, end - 1);
    let n = raw.len();
    let mut out = "";
    let mut k = 0;
    while (k < n) {
        let c = raw.code_at(k);
        if (c != C_BACKSLASH) {
            out = f"{out}{raw.slice(k, k + 1)}";
            k = k + 1;
            continue;
        }
        if (k + 1 >= n) {
            break;
        }
        let e = raw.code_at(k + 1);
        if (e == C_QUOTE) {
            out = f"{out}\"";
        } else if (e == C_BACKSLASH) {
            out = f"{out}\\";
        } else if (e == C_SLASH) {
            out = f"{out}/";
        } else if (e == 110) {
            out = f"{out}\n";
        } else if (e == 116) {
            out = f"{out}\t";
        } else if (e == 117) {
            out = f"{out}\u{FFFD}";
            k = k + 6;
            continue;
        } else {
            out = f"{out}{raw.slice(k + 1, k + 2)}";
        }
        k = k + 2;
    }
    return out;
}

fn json_skip(t: str, i: i32) -> i32 {
    let n = t.len();
    let j = skip_ws(t, i);
    if (j >= n) {
        return n;
    }
    let c = t.code_at(j);
    if (c == C_QUOTE) {
        return str_end(t, j);
    }
    if (c == C_LBRACE || c == C_LBRACKET) {
        let mut depth = 0;
        let mut k = j;
        while (k < n) {
            let d = t.code_at(k);
            if (d == C_QUOTE) {
                k = str_end(t, k);
                continue;
            }
            if (d == C_LBRACE || d == C_LBRACKET) {
                depth = depth + 1;
            } else if (d == C_RBRACE || d == C_RBRACKET) {
                depth = depth - 1;
                if (depth == 0) {
                    return k + 1;
                }
            }
            k = k + 1;
        }
        return n;
    }
    let mut k = j;
    while (k < n) {
        let d = t.code_at(k);
        if (d == C_COMMA || d == C_RBRACE || d == C_RBRACKET || ws(d)) {
            break;
        }
        k = k + 1;
    }
    return k;
}

fn json_get_str(t: str, key: str) -> str {
    let n = t.len();
    let mut i = skip_ws(t, 0);
    if (i >= n || t.code_at(i) != C_LBRACE) {
        return "";
    }
    i = skip_ws(t, i + 1);
    while (i < n) {
        if (t.code_at(i) == C_RBRACE) {
            return "";
        }
        if (t.code_at(i) != C_QUOTE) {
            return "";
        }
        let key_end = str_end(t, i);
        let k = t.slice(i + 1, key_end - 1);
        let j = skip_ws(t, key_end);
        if (j >= n || t.code_at(j) != C_COLON) {
            return "";
        }
        let vs = skip_ws(t, j + 1);
        if (k == key) {
            if (vs < n && t.code_at(vs) == C_QUOTE) {
                return str_unquote(t, vs);
            }
            if (vs < n && t.code_at(vs) != C_LBRACE && t.code_at(vs) != C_LBRACKET) {
                let vend = json_skip(t, vs);
                return t.slice(vs, vend);
            }
            return "";
        }
        i = skip_ws(t, json_skip(t, vs));
        if (i < n && t.code_at(i) == C_COMMA) {
            i = skip_ws(t, i + 1);
        }
    }
    return "";
}
"#;

fn setup() -> TurTestApp {
    let mut app = TurTestApp::new(300.0, 100.0).unwrap();
    app.load_rut_module(PROBE_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app
}

#[test]
fn full_name_extracts_verbatim() {
    let mut app = setup();
    app.call_rut_entry("probe", app.rut_start_answer(), 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(
        app.query_text(&["ghj-full"]).as_deref(),
        Some("facebook/react"),
        "the case's scanner must extract full_name verbatim"
    );
    assert_eq!(
        app.query_text(&["ghj-desc"]).as_deref(),
        Some("The library for web and native user interfaces."),
    );
}
