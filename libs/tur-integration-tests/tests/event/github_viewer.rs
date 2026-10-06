//! The `github-viewer` corpus case: the net-riding showcase (landing →
//! meta fetch → contents fetch → explorer, with visible loading + error
//! states). The case needs the net rows, so every app here is built via
//! `new_with_http` — the same capability set the website runtime registers
//! (tur-net-wasm). Journeys are canned through `RecordingHttp`'s ordered
//! responses; on an empty script the transport error state is what
//! renders (the degrade path the offline browser shows).

use std::time::Duration;

use tur_integration_tests::{TurTestApp, text_response};

const META: &str = "{\"full_name\":\"facebook/react\",\"description\":\"The library for web and native user interfaces.\",\"stargazers_count\":234000,\"forks_count\":48000,\"open_issues_count\":1000,\"language\":\"TypeScript\"}";

const ROOT_CONTENTS: &str = "[{\"name\":\"packages\",\"path\":\"packages\",\"size\":0,\"type\":\"dir\"},{\"name\":\"README.md\",\"path\":\"README.md\",\"size\":2739,\"type\":\"file\"}]";

const PACKAGES_CONTENTS: &str =
    "[{\"name\":\"react\",\"path\":\"packages/react\",\"size\":0,\"type\":\"dir\"}]";

fn build() -> TurTestApp {
    let mut app = TurTestApp::new_with_http(500.0, 700.0).unwrap();
    app.load_rut_bundle("github-viewer").unwrap();
    app
}

/// Drive frames until `probe` holds (the journey awaits ride the worker's
/// capability lane; the kit prelude compiles on the real clock).
fn wait_for(app: &TurTestApp, probe: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if probe() {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
        app.wait_for_timeout(Duration::from_millis(32));
    }
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    use tur_engine::core::element::ElementNodeId;
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let (cx, cy) = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .center();
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

#[test]
fn github_viewer_lands_with_the_form_and_chips() {
    let app = build();
    assert!(
        app.query_element(&["gh-input"]).is_some(),
        "the draft field"
    );
    assert!(
        app.query_element(&["gh-browse"]).is_some(),
        "the browse button"
    );
    assert!(
        app.query_element(&["gh-sug-2"]).is_some(),
        "the last quick-pick chip"
    );
}

#[test]
fn github_viewer_shows_the_error_state_when_offline() {
    let mut app = build();
    // No canned responses — the transport fails exactly like an offline
    // browser. The suggestion chip takes the real user path (no typing).
    click_qk(&mut app, &["gh-sug-0"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-err"])
            .is_some_and(|t| t.contains("offline"))
    });
    assert!(
        ok,
        "the offline banner should appear: {:?}",
        app.query_text(&["gh-err"])
    );
    // The failure falls back to the landing (the form is still up).
    assert!(
        app.query_element(&["gh-browse"]).is_some(),
        "back on the landing"
    );
}

#[test]
fn github_viewer_rejects_a_malformed_draft() {
    let mut app = build();
    // The landing boots with the draft pre-filled — clear it first (click
    // into the field, select all, delete), then Browse with an empty
    // draft: the inline validation error.
    click_qk(&mut app, &["gh-input"]);
    app.send_key_with_modifiers("a", false, true);
    app.send_key("Backspace");
    app.wait_for_timeout(Duration::ZERO);
    click_qk(&mut app, &["gh-browse"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-err"])
            .is_some_and(|t| t.contains("owner/name"))
    });
    assert!(ok, "the draft error should appear");
    assert!(
        !app.query_text(&["gh-err"]).unwrap().contains("offline"),
        "no fetch fired for a malformed draft"
    );
}

#[test]
fn github_viewer_opens_a_repo_into_the_explorer() {
    let mut app = build();
    app.set_http_responses(vec![
        text_response(200, META),
        text_response(200, ROOT_CONTENTS),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-crumb"]).as_deref() == Some("facebook/react")
    });
    assert!(
        ok,
        "the explorer should open: {:?}",
        app.query_text(&["gh-crumb"])
    );
    assert_eq!(
        app.query_text(&["gh-stats"]).as_deref(),
        Some("TypeScript · stars 234.0k · forks 48.0k · issues 1.0k"),
    );
    assert_eq!(
        app.query_text(&["gh-desc"]).as_deref(),
        Some("The library for web and native user interfaces."),
    );
    // Dirs sort first: the first row is `packages`, then the README.
    assert!(
        app.query_element(&["gh-row"]).is_some(),
        "the listing rows render"
    );
}

#[test]
fn github_viewer_descends_into_a_directory() {
    let mut app = build();
    app.set_http_responses(vec![
        text_response(200, META),
        text_response(200, ROOT_CONTENTS),
        text_response(200, PACKAGES_CONTENTS),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    wait_for(&app, || {
        app.query_text(&["gh-crumb"]).as_deref() == Some("facebook/react")
    });
    // Tap the first row (the `packages` directory) — a fresh contents
    // fetch for the nested path.
    click_qk(&mut app, &["gh-row"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-path"]).as_deref() == Some("packages")
    });
    assert!(ok, "the path breadcrumb should descend");
}

#[test]
fn github_viewer_selects_a_file() {
    let mut app = build();
    let files_only =
        "[{\"name\":\"README.md\",\"path\":\"README.md\",\"size\":2739,\"type\":\"file\"}]";
    app.set_http_responses(vec![
        text_response(200, META),
        text_response(200, files_only),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    wait_for(&app, || {
        app.query_text(&["gh-crumb"]).as_deref() == Some("facebook/react")
    });
    click_qk(&mut app, &["gh-row"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-selected"])
            .is_some_and(|t| t.contains("README.md"))
    });
    assert!(ok, "the file selection should fill the footer");
    assert_eq!(
        app.query_text(&["gh-selected"]).as_deref(),
        Some("README.md — 2.6 KB"),
    );
}

#[test]
fn github_viewer_crumb_keeps_the_parsed_draft_identity() {
    // The round-5 audit caught the crumb reading "react/react": open_repo
    // used to overwrite the crumb atom with the API's `full_name`. Boa's
    // repo$ keeps the PARSED draft identity — the API name never wins, so
    // even a poisoned payload can't bend the crumb.
    let mut app = build();
    let poisoned = "{\"full_name\":\"WRONG/WRONG\",\"description\":\"x\",\"stargazers_count\":1,\"forks_count\":1,\"open_issues_count\":1,\"language\":\"Rust\"}";
    app.set_http_responses(vec![
        text_response(200, poisoned),
        text_response(200, ROOT_CONTENTS),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-crumb"]).is_some_and(|t| !t.is_empty())
    });
    assert!(ok, "the explorer should open");
    assert_eq!(
        app.query_text(&["gh-crumb"]).as_deref(),
        Some("facebook/react"),
        "the crumb is the parsed draft, not the API full_name"
    );
    // The stats still parse FROM the payload (the poisoned language).
    assert!(
        app.query_text(&["gh-stats"]).unwrap().starts_with("Rust"),
        "the metadata itself still comes from the API: {:?}",
        app.query_text(&["gh-stats"])
    );
}

#[test]
fn github_viewer_crumb_navigates_to_the_root() {
    // Descend into `packages`, then click the crumb chip: the path resets
    // and a fresh ROOT listing loads (the boa navigateToRoot).
    let mut app = build();
    app.set_http_responses(vec![
        text_response(200, META),
        text_response(200, ROOT_CONTENTS),
        text_response(200, PACKAGES_CONTENTS),
        text_response(200, ROOT_CONTENTS),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    wait_for(&app, || {
        app.query_text(&["gh-crumb"]).as_deref() == Some("facebook/react")
    });
    click_qk(&mut app, &["gh-row"]);
    wait_for(&app, || {
        app.query_text(&["gh-path"]).as_deref() == Some("packages")
    });
    click_qk(&mut app, &["gh-crumb"]);
    let ok = wait_for(&app, || {
        app.query_text(&["gh-path"]).as_deref() == Some("")
    });
    assert!(ok, "the crumb tap returns to the root listing");
    // The root listing re-loaded (the README row is back under the
    // packages-less root view).
    assert!(
        app.query_element(&["gh-row"]).is_some(),
        "the root listing rendered after the crumb navigation"
    );
    // Settled: the busy "Loading…" rail is not on screen.
    assert!(
        app.query_element(&["gh-loading"]).is_none(),
        "the list-area loading rail clears when the fetch settles"
    );
}

#[test]
fn github_viewer_has_no_full_pane_loading_mode() {
    // The round-4 shape swapped the whole body to a "Loading…" pane on
    // open; boa goes straight to the explorer chrome with the loading
    // state in the LIST AREA. The "loading" mode is gone — the explorer
    // is the only non-landing body.
    let mut app = build();
    app.set_http_responses(vec![
        text_response(200, META),
        text_response(200, ROOT_CONTENTS),
    ]);
    click_qk(&mut app, &["gh-sug-0"]);
    let ok = wait_for(&app, || {
        app.query_element(&["gh-crumb"]).is_some()
    });
    assert!(ok, "the explorer chrome mounts");
    assert!(
        app.query_element(&["gh-loading"]).is_none(),
        "no loading rail once the fetch settles"
    );
    assert!(
        app.query_element(&["gh-row"]).is_some(),
        "the listing renders"
    );
}
