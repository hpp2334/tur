//! The std json offering gate: modules compiled against the engine's
//! offered set can `use json::{ .. }` — rut's std serde package
//! (vendored from the pinned rut checkout, offered through
//! `TurStdPlugin`'s std-json rail with its `strbuild`/`pouch`/`nmapset`
//! dep closure + the two `*_host` body pkgs installed from `rut-std`).
//!
//! What this pins:
//! - the pkg is OFFERED and rides the kit bundle LINKED (compiled at
//!   pack time — no per-session source compile),
//! - consumer-typed instantiation crosses via the LOCAL SHIM (a
//!   consumer-side generic fn over `T::decode` — json's own `decodeJson<T>`
//!   is owner-anchored and unseedable from the packed unit),
//! - a consumer record satisfies `JsonDeserialize` structurally (the
//!   decode member on its own inherent impl),
//! - the (?T, ?E) pair law: success and failure both land.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// The happy path + the nullable-field default + unknown-key skipping:
/// `description` is null, `language` is absent, `owner` is unknown to the
/// schema (skipped), and `stars` carries the i64 lane.
const DECODE_RUT: &str = r#"
use json::{ DecodeErrorKind, DecodeJsonError, JsonDeserialize, JsonReader };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ sv };
use tur_kit::text::core::{ Text };

struct Repo {
    name: str;
    desc: str;
    stars: i64;
}

impl Repo {
    pub fn decode(mut r: JsonReader) -> (?Self, ?DecodeJsonError) {
        r.begin_object();
        let mut name: ?str = nil;
        let mut desc: ?str = nil;
        let mut stars: ?i64 = nil;
        while (r.more()) {
            let k = r.next_key();
            if (r.failed()) { return (nil, r.error()); }
            when (k) {
                "full_name" -> {
                    let v = r.read_str();
                    if (r.failed()) { return (nil, r.error()); }
                    name = v;
                },
                "description" -> {
                    if (r.try_null()) {
                        desc = nil;
                    } else {
                        if (r.failed()) { return (nil, r.error()); }
                        let v = r.read_str();
                        if (r.failed()) { return (nil, r.error()); }
                        desc = v;
                    }
                },
                "stargazers_count" -> {
                    let v = r.read_i64();
                    if (r.failed()) { return (nil, r.error()); }
                    stars = v;
                },
                else -> { r.skip_value(); },
            }
            if (r.failed()) { return (nil, r.error()); }
        }
        r.end_object();
        if (r.failed()) { return (nil, r.error()); }
        if (name == nil || stars == nil) {
            return (nil, DecodeJsonError { kind: DecodeErrorKind::WrongType, at: 0, got: "", expected: "full_name/stargazers_count" });
        }
        let nm: str = name;
        let st: i64 = stars;
        let mut ds: str = "";
        if (desc != nil) {
            let v: str = desc;
            ds = v;
        }
        return (Repo { name: nm, desc: ds, stars: st }, nil);
    }
}


// The local decode shim — the upstream CLI example's pattern. json rides
// the kit bundle LINKED, and its generic bodies are owner-anchored
// (sealed at json's pack time); a LOCAL generic fn monomorphizes in THIS
// unit, `T::decode(r)` dispatches the record's own decode member.
fn decode_str<T requires JsonDeserialize>(s: str) -> (?T, ?DecodeJsonError) {
    let mut r = JsonReader::of(s);
    let (v, e) = T::decode(r);
    if (e != nil) { return (nil, e); }
    if (!r.at_end()) {
        let at = r.here();
        let got = r.rest();
        return (nil, DecodeJsonError { kind: DecodeErrorKind::Trailing, at: at as i64, got: got, expected: "end of input" });
    }
    return (v, nil);
}

entry fn start() -> u64 {
    let payload = "{\"full_name\":\"facebook/react\",\"description\":null,\"owner\":{\"login\":\"facebook\"},\"stargazers_count\":231000}";
    let (repo, e) = decode_str<Repo>(payload);
    let label = if (e != nil) {
        "decode failed"
    } else {
        f"{repo.name}|{repo.desc}|{repo.stars}"
    };
    mount(Column()
        .child(Text().text(sv(label)).query_key("rut/text").build())
        .build());
    return 0;
}
"#;

/// The failure rail: a 404-shaped payload (no full_name) decodes to the
/// WrongType error — the case's degradation story rides the pair law.
const ERROR_RUT: &str = r#"
use json::{ DecodeErrorKind, DecodeJsonError, JsonDeserialize, JsonReader };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ sv };
use tur_kit::text::core::{ Text };

struct Repo {
    name: str;
    stars: i64;
}

impl Repo {
    pub fn decode(mut r: JsonReader) -> (?Self, ?DecodeJsonError) {
        r.begin_object();
        let mut name: ?str = nil;
        let mut stars: ?i64 = nil;
        while (r.more()) {
            let k = r.next_key();
            if (r.failed()) { return (nil, r.error()); }
            when (k) {
                "full_name" -> {
                    let v = r.read_str();
                    if (r.failed()) { return (nil, r.error()); }
                    name = v;
                },
                "stargazers_count" -> {
                    let v = r.read_i64();
                    if (r.failed()) { return (nil, r.error()); }
                    stars = v;
                },
                else -> { r.skip_value(); },
            }
            if (r.failed()) { return (nil, r.error()); }
        }
        r.end_object();
        if (r.failed()) { return (nil, r.error()); }
        if (name == nil || stars == nil) {
            return (nil, DecodeJsonError { kind: DecodeErrorKind::WrongType, at: 0, got: "", expected: "full_name" });
        }
        let nm: str = name;
        let st: i64 = stars;
        return (Repo { name: nm, stars: st }, nil);
    }
}


// The local decode shim — the upstream CLI example's pattern. json rides
// the kit bundle LINKED, and its generic bodies are owner-anchored
// (sealed at json's pack time); a LOCAL generic fn monomorphizes in THIS
// unit, `T::decode(r)` dispatches the record's own decode member.
fn decode_str<T requires JsonDeserialize>(s: str) -> (?T, ?DecodeJsonError) {
    let mut r = JsonReader::of(s);
    let (v, e) = T::decode(r);
    if (e != nil) { return (nil, e); }
    if (!r.at_end()) {
        let at = r.here();
        let got = r.rest();
        return (nil, DecodeJsonError { kind: DecodeErrorKind::Trailing, at: at as i64, got: got, expected: "end of input" });
    }
    return (v, nil);
}

entry fn start() -> u64 {
    let payload = "{\"message\":\"Not Found\",\"documentation_url\":\"https://docs.github.com/rest\"}";
    let (repo, e) = decode_str<Repo>(payload);
    let label = if (e != nil) {
        f"err:{e.kind}"
    } else {
        f"{repo.name}"
    };
    mount(Column()
        .child(Text().text(sv(label)).query_key("rut/text").build())
        .build());
    return 0;
}
"#;

/// Two records + two shim instantiations in ONE module — the
/// github-viewer shape (RepoPayload + ApiError). Pins the instantiation
/// ledger across distinct consumer types.
const TWO_RECORDS_RUT: &str = r#"
use json::{ DecodeErrorKind, DecodeJsonError, JsonDeserialize, JsonReader };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ sv };
use tur_kit::text::core::{ Text };

struct Alpha {
    tag: str;
}

impl Alpha {
    pub fn decode(mut r: JsonReader) -> (?Self, ?DecodeJsonError) {
        r.begin_object();
        let mut tag: ?str = nil;
        while (r.more()) {
            let k = r.next_key();
            if (r.failed()) { return (nil, r.error()); }
            when (k) {
                "tag" -> {
                    let v = r.read_str();
                    if (r.failed()) { return (nil, r.error()); }
                    tag = v;
                },
                else -> { r.skip_value(); },
            }
            if (r.failed()) { return (nil, r.error()); }
        }
        r.end_object();
        if (r.failed()) { return (nil, r.error()); }
        if (tag == nil) { return (nil, nil); }
        let t: str = tag;
        return (Alpha { tag: t }, nil);
    }
}

struct Beta {
    note: str;
}

impl Beta {
    pub fn decode(mut r: JsonReader) -> (?Self, ?DecodeJsonError) {
        r.begin_object();
        let mut note: ?str = nil;
        while (r.more()) {
            let k = r.next_key();
            if (r.failed()) { return (nil, r.error()); }
            when (k) {
                "note" -> {
                    let v = r.read_str();
                    if (r.failed()) { return (nil, r.error()); }
                    note = v;
                },
                else -> { r.skip_value(); },
            }
            if (r.failed()) { return (nil, r.error()); }
        }
        r.end_object();
        if (r.failed()) { return (nil, r.error()); }
        if (note == nil) { return (nil, nil); }
        let n: str = note;
        return (Beta { note: n }, nil);
    }
}


// The local decode shim — the upstream CLI example's pattern. json rides
// the kit bundle LINKED, and its generic bodies are owner-anchored
// (sealed at json's pack time); a LOCAL generic fn monomorphizes in THIS
// unit, `T::decode(r)` dispatches the record's own decode member.
fn decode_str<T requires JsonDeserialize>(s: str) -> (?T, ?DecodeJsonError) {
    let mut r = JsonReader::of(s);
    let (v, e) = T::decode(r);
    if (e != nil) { return (nil, e); }
    if (!r.at_end()) {
        let at = r.here();
        let got = r.rest();
        return (nil, DecodeJsonError { kind: DecodeErrorKind::Trailing, at: at as i64, got: got, expected: "end of input" });
    }
    return (v, nil);
}

entry fn start() -> u64 {
    let (a, ea) = decode_str<Alpha>("{\"tag\":\"x\"}");
    let (b, eb) = decode_str<Beta>("{\"note\":\"y\"}");
    let ta = if (ea != nil) { "errA" } else if (a == nil) { "nilA" } else { a.tag };
    let tb = if (eb != nil) { "errB" } else if (b == nil) { "nilB" } else { b.note };
    mount(Column()
        .child(Text().text(sv(f"{ta}/{tb}")).query_key("rut/text").build())
        .build());
    return 0;
}
"#;

fn rut_text_of(app: &TurTestApp) -> String {
    let id = app
        .query_element(&["rut", "text"])
        .expect("bound text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn std_json_decodes_two_records_in_one_module() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(TWO_RECORDS_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_text_of(&app),
        "x/y",
        "two distinct consumer types instantiate the shim in one unit"
    );
}

#[test]
fn std_json_decodes_a_repo_record() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(DECODE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_text_of(&app),
        "facebook/react||231000",
        "the decode landed: name, the null description as empty, the stars"
    );
}

#[test]
fn std_json_surfaces_the_error_pair() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(ERROR_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_text_of(&app),
        "err:WrongType",
        "a 404-shaped payload answers (nil, err) — the pair law"
    );
}
