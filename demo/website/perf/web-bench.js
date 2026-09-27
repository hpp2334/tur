// Web render-performance bench — real browser, real WebGL2 raster path.
//
// Run: start the dev server (`cd demo/website && pnpm dev`), open
// https://localhost:8080 with agent-browser (`--ignore-https-errors`), then
// `agent-browser eval "$(cat demo/website/perf/web-bench.js)"`. Prints a
// summary via console.log and resolves to the results object (readable with
// `--json`).
//
// Uses the engine's frame-stats probe (`turDevTool.frameStats()`) surfaced to
// the page by `TurWebsiteApp.dev_tool()`:
//   - worker side: painted frames, per-frame walk/record counters + timings
//     (cumulative `totals` — deltas over the measurement window isolate the
//     bench frames),
//   - host side (opt-in `setHostFrameTiming(true)`): `applyUs` (scene rebuild
//     + command playback) and `presentUs` (sparse-strip raster + GL
//     composite) of the most recent APPLIED frame (`lastHost`).
//
// Scenarios:
//   1. animated  — 50 static cells under an infinite opacity animation: the
//      worst case (every frame differs; record + ship + rebuild + raster
//      every frame).
//   2. static-identical — a colored container + a stream of synthetic
//      pointer-moves (every PointerMove marks the frame paint-worthy). The
//      worker repaints each time (paintedFrames advances) but the render
//      commit point fingerprints identical batches and skips re-render —
//      visible as lastHost.frame stalling while paintedFrames climbs.
//   3. editor-4000 — a 4000-line spanned `Input` (monospace) in a
//      `ScrollView`, scrolled by a real wheel-event stream. Time-to-first-
//      frame captures the one-time 4000-line text-layout cost.
//   4. image-list — a 400-item `LazyList` (48×48 image + two text lines per
//      64px row), scrolled by the same wheel stream. Exercises lazy item
//      build + image draws on the hot path.
//
// Scrolling is driven with synthetic `WheelEvent`s on the canvas — the real
// input path (listener → engine PlatformEvent → scroll subsystem), paced by
// requestAnimationFrame so one event goes out per display tick.
//
// Scenario filter: set `globalThis.__benchScenarios` (comma-separated names:
// animated, staticIdentical, editor4000, imageList) before eval'ing to run a
// subset. With a filter set the sweep runs in the PAGE and results land in
// `globalThis.__turBench` (`{ done, results, error }`) — follow-up evals poll
// that handle instead of blocking on one long eval (agent-browser evals
// time out on the full ~30 s sweep). Without a filter the IIFE runs
// everything and resolves to the results directly.
//
// Note: loadAndRunModule replaces the mounted module (the playground root is
// torn down) — reload the page afterwards to restore the playground.

(async () => {
    const SCENARIOS = (
        globalThis.__benchScenarios ??
        "animated,staticIdentical,editor4000,imageList"
    ).split(",");
    const SAMPLE_MS = 6000;
    const MOVES = 120;
    const WHEEL_EVENTS = 90; // rAF-paced: ~1.5 s of scrolling at 60fps
    const WHEEL_DELTA_Y = 80;

    const ANIMATED_SOURCE = `
import { mount, Column, Container, Opacity, Color, derive, mutate, source } from "tur:std";
import { createAnimationController } from "tur:animation";

const opacity$ = source(1);

const ctrl = createAnimationController({
    duration: 1000,
    repeat: "infinite",
    onTick: mutate((ctx, v) => { ctx.set(opacity$, v); }),
});

const kids = [];
for (let i = 0; i < 50; i++) {
    kids.push(Container().width(40).height(40).color(Color.hex("#3060c0")).build());
}

export function start() {
    mount(Opacity().value(derive((ctx) => ctx.get(opacity$))).child(
        Column().children(kids).build(),
    ).build());
    ctrl.forward();
}
`;

    const STATIC_SOURCE = `
import { mount, Container, createColor } from "tur:std";

export function start() {
    mount(Container()
        .width(120)
        .height(120)
        .color(createColor(255, 0, 0, 255))
        .build());
}
`;

    const EDITOR_4000_SOURCE = `
import {
    mount,
    ScrollView,
    Input,
    createTextEditingController,
    createScrollController,
} from "tur:std";

const LINES = 4000;
const spans = [];
for (let i = 0; i < LINES; i++) {
    spans.push({ content: "const value" + i + " = " + i + "; // line " + i + "\\n" });
}
const ctrl = createTextEditingController();
ctrl.setSpans(spans);
const scroller = createScrollController({ initialOffset: 0 });

export function start() {
    mount(ScrollView()
        .controller(scroller)
        .child(Input()
            .controller(ctrl)
            .multiline(true)
            .fontFamily("monospace")
            .fontSize(14)
            .build())
        .build());
}
`;

    // 64×64 RGBA PNG — a horizontal gradient (identical rows, so zlib crushed
    // it to 392 bytes). Inline byte literal: the module is evaluated in the
    // ENGINE realm (boa), which has no `atob`.
    const IMG_PNG_BYTES = `[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 64,
        0, 0, 0, 64, 8, 6, 0, 0, 0, 170, 105, 113, 222, 0, 0, 1, 79, 73, 68, 65,
        84, 120, 218, 237, 208, 129, 102, 22, 0, 0, 70, 209, 159, 69, 99, 81, 52, 26, 141, 162,
        177, 209, 216, 104, 20, 141, 69, 163, 104, 108, 108, 20, 139, 70, 163, 88, 44, 22, 139, 98,
        151, 141, 162, 177, 81, 20, 27, 27, 27, 27, 69, 81, 20, 139, 141, 198, 98, 163, 40, 54,
        22, 27, 197, 198, 162, 88, 20, 27, 183, 231, 24, 223, 121, 132, 83, 40, 160, 251, 216, 177,
        152, 191, 30, 96, 219, 67, 252, 178, 148, 45, 203, 216, 176, 156, 239, 30, 103, 205, 10, 86,
        173, 98, 217, 106, 190, 88, 203, 39, 235, 88, 244, 12, 11, 214, 243, 193, 115, 204, 218, 200,
        140, 23, 121, 107, 19, 175, 109, 225, 165, 109, 60, 247, 10, 83, 94, 101, 194, 14, 198, 236,
        100, 196, 155, 60, 245, 22, 143, 189, 205, 144, 119, 120, 228, 93, 30, 120, 159, 126, 161, 207,
        1, 238, 249, 144, 94, 7, 233, 113, 152, 110, 159, 208, 229, 51, 110, 56, 202, 117, 199, 185,
        230, 36, 237, 78, 115, 217, 23, 180, 250, 138, 102, 223, 112, 201, 119, 92, 240, 61, 231, 157,
        163, 193, 121, 206, 250, 145, 211, 46, 113, 202, 207, 212, 248, 149, 147, 174, 80, 233, 55, 78,
        184, 206, 49, 127, 112, 212, 77, 142, 248, 147, 195, 254, 230, 160, 127, 40, 241, 31, 251, 221,
        165, 200, 66, 2, 18, 144, 128, 4, 36, 32, 1, 9, 72, 64, 2, 18, 144, 128, 4, 36,
        32, 1, 9, 72, 64, 2, 18, 144, 128, 4, 36, 32, 1, 9, 72, 64, 2, 18, 144, 128,
        4, 36, 32, 1, 9, 72, 64, 2, 18, 144, 128, 4, 36, 32, 1, 9, 72, 64, 2, 18,
        144, 128, 4, 36, 32, 1, 9, 72, 64, 2, 18, 144, 128, 4, 36, 32, 1, 9, 72, 64, 2,
        18, 144, 128, 4, 36, 32, 1, 123, 63, 224, 63, 50, 181, 226, 74, 106, 64, 203, 228,
        0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]`;

    const IMAGE_LIST_SOURCE = `
import {
    mount,
    LazyList,
    Container,
    Row,
    Column,
    Text,
    Image,
    createImageResource,
} from "tur:std";

const ITEMS = 400;
const ROW_HEIGHT = 64;

const resource = createImageResource(new Uint8Array(${IMG_PNG_BYTES}));

export function start() {
    mount(LazyList()
        .itemCount(ITEMS)
        .builder((i) =>
            Container()
                .height(ROW_HEIGHT)
                .children([
                    Row()
                        .children([
                            Image().resourceId(resource).width(48).height(48).fit(0).build(),
                            Column().children([
                                Text().text("Item " + i).fontSize(15).build(),
                                Text().text("Subtitle for row " + i).fontSize(12).build(),
                            ]).build(),
                        ])
                        .build(),
                ])
                .build())
        .build());
}
`;

    if (
        typeof globalThis.turDevTool === "undefined" ||
        typeof globalThis.turApp === "undefined"
    ) {
        throw new Error(
            "tur not booted yet — wait for the playground to load first",
        );
    }

    const frameStats = async () =>
        JSON.parse(await globalThis.turDevTool.frameStats());
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const raf = () => new Promise((r) => requestAnimationFrame(r));

    const delta = (after, before) => ({
        flushes: after.flushes - before.flushes,
        paintedFrames: after.paintedFrames - before.paintedFrames,
        flushUs: after.totals.flushUs - before.totals.flushUs,
        nodesWalked: after.totals.nodesWalked - before.totals.nodesWalked,
        opsRecorded: after.totals.opsRecorded - before.totals.opsRecorded,
    });

    const perFrame = (count, frames) =>
        +(count / Math.max(frames, 1)).toFixed(1);

    // Load a module and resolve once its first frame has painted; returns the
    // one-time load→first-paint latency (text layout of 4000 lines, initial
    // list build, …) — a distinct number from the steady-state rates below.
    // Capped at 15 s; `null` means "no frame within the cap".
    const loadModule = async (source) => {
        const before = await frameStats();
        const t0 = performance.now();
        await globalThis.turApp.loadAndRunModule(source);
        for (;;) {
            if (performance.now() - t0 > 15000) return null;
            await sleep(100);
            const now = await frameStats();
            if (now.paintedFrames > before.paintedFrames) break;
        }
        return Math.round(performance.now() - t0);
    };

    // A rAF-paced wheel stream at the viewport center — the real input path
    // (canvas listener → engine PlatformEvent → scroll subsystem).
    const wheelScroll = async (count, deltaY) => {
        const canvas = document.querySelector("canvas");
        const cx = Math.round(window.innerWidth / 2);
        const cy = Math.round(window.innerHeight / 2);
        for (let i = 0; i < count; i++) {
            canvas.dispatchEvent(
                new WheelEvent("wheel", {
                    deltaY,
                    clientX: cx,
                    clientY: cy,
                    bubbles: true,
                    cancelable: true,
                }),
            );
            await raf();
        }
    };

    const results = {};
    const want = (name) => SCENARIOS.includes(name);

    const runAll = async () => {
        await globalThis.turDevTool.setHostFrameTiming(true);

        // ── Scenario 1: animated (every frame differs) ──────────────────
        if (want("animated")) {
            await loadModule(ANIMATED_SOURCE);
            await sleep(1500); // settle into the steady state
            const before = await frameStats();
            await sleep(SAMPLE_MS);
            const after = await frameStats();
            const d = delta(after, before);
            results.animated = {
                windowMs: SAMPLE_MS,
                painted: d.paintedFrames,
                workerUsPerFrame: perFrame(d.flushUs, d.paintedFrames),
                nodesPerFrame: perFrame(d.nodesWalked, d.paintedFrames),
                opsPerFrame: perFrame(d.opsRecorded, d.paintedFrames),
                host: after.lastHost,
            };
            console.log(
                `animated (50 cells, infinite)   painted=${d.paintedFrames}` +
                    `  worker=${results.animated.workerUsPerFrame}us/frame` +
                    `  nodes/frame=${results.animated.nodesPerFrame}` +
                    `  ops/frame=${results.animated.opsPerFrame}` +
                    (after.lastHost
                        ? `  host apply=${after.lastHost.applyUs}us present=${after.lastHost.presentUs}us`
                        : "  host: no frame applied!"),
            );
        }

        // ── Scenario 2: static + identical repaint stream (dedup evidence) ──
        if (want("staticIdentical")) {
            await loadModule(STATIC_SOURCE);
            await sleep(1500);
            const before = await frameStats();

            for (let i = 0; i < MOVES; i++) {
                document.querySelector("canvas").dispatchEvent(
                    new MouseEvent("mousemove", {
                        clientX: 150,
                        clientY: 150,
                        bubbles: true,
                    }),
                );
                if (i % 10 === 9) await sleep(16);
            }
            await sleep(1500);

            const after = await frameStats();
            const d = delta(after, before);
            const hostRenders =
                (after.lastHost?.frame ?? 0) - (before.lastHost?.frame ?? 0);
            results.staticIdentical = {
                syntheticMoves: MOVES,
                workerRepaints: d.paintedFrames,
                hostRenders,
                dedupSkipped: d.paintedFrames - hostRenders,
                workerUsPerFrame: perFrame(d.flushUs, d.paintedFrames),
            };
            console.log(
                `static-identical (${MOVES} moves)  repaints=${d.paintedFrames}` +
                    `  hostRenders=${hostRenders}` +
                    `  dedupSkipped=${results.staticIdentical.dedupSkipped}` +
                    `  worker=${results.staticIdentical.workerUsPerFrame}us/frame`,
            );
        }

        // ── Scenario 3: 4000-line editor, scrolled by a wheel stream ────────
        if (want("editor4000")) {
            const firstFrameMs = await loadModule(EDITOR_4000_SOURCE);
            await sleep(1500);
            const before = await frameStats();
            await wheelScroll(WHEEL_EVENTS, WHEEL_DELTA_Y);
            await sleep(1000);
            const after = await frameStats();
            const d = delta(after, before);
            results.editor4000 = {
                lines: 4000,
                firstFrameMs,
                wheelEvents: WHEEL_EVENTS,
                painted: d.paintedFrames,
                workerUsPerFrame: perFrame(d.flushUs, d.paintedFrames),
                nodesPerFrame: perFrame(d.nodesWalked, d.paintedFrames),
                opsPerFrame: perFrame(d.opsRecorded, d.paintedFrames),
                host: after.lastHost,
            };
            console.log(
                `editor-4000 (wheel scroll)      firstFrame=${firstFrameMs}ms` +
                    `  painted=${d.paintedFrames}` +
                    `  worker=${results.editor4000.workerUsPerFrame}us/frame` +
                    `  nodes/frame=${results.editor4000.nodesPerFrame}` +
                    `  ops/frame=${results.editor4000.opsPerFrame}` +
                    (after.lastHost
                        ? `  host apply=${after.lastHost.applyUs}us present=${after.lastHost.presentUs}us`
                        : "  host: no frame applied!"),
            );
        }

        // ── Scenario 4: 400-item image list, scrolled by a wheel stream ─────
        if (want("imageList")) {
            const firstFrameMs = await loadModule(IMAGE_LIST_SOURCE);
            await sleep(1500);
            const before = await frameStats();
            await wheelScroll(WHEEL_EVENTS, WHEEL_DELTA_Y);
            await sleep(1000);
            const after = await frameStats();
            const d = delta(after, before);
            results.imageList = {
                items: 400,
                rowHeight: 64,
                firstFrameMs,
                wheelEvents: WHEEL_EVENTS,
                painted: d.paintedFrames,
                workerUsPerFrame: perFrame(d.flushUs, d.paintedFrames),
                nodesPerFrame: perFrame(d.nodesWalked, d.paintedFrames),
                opsPerFrame: perFrame(d.opsRecorded, d.paintedFrames),
                host: after.lastHost,
            };
            console.log(
                `image-list (400 items, scroll)  firstFrame=${firstFrameMs}ms` +
                    `  painted=${d.paintedFrames}` +
                    `  worker=${results.imageList.workerUsPerFrame}us/frame` +
                    `  nodes/frame=${results.imageList.nodesPerFrame}` +
                    `  ops/frame=${results.imageList.opsPerFrame}` +
                    (after.lastHost
                        ? `  host apply=${after.lastHost.applyUs}us present=${after.lastHost.presentUs}us`
                        : "  host: no frame applied!"),
            );
        }

        // Restore host-timing off (bench hygiene).
        await globalThis.turDevTool.setHostFrameTiming(false);
        return results;
    };

    if (globalThis.__benchScenarios) {
        // Fire-and-poll mode: run the sweep in the page, publish to
        // `globalThis.__turBench`. Each follow-up eval reads that handle in
        // milliseconds instead of blocking the daemon for the whole sweep.
        globalThis.__turBench = { done: false, results: null, error: null };
        runAll()
            .then((r) =>
                Object.assign(globalThis.__turBench, {
                    done: true,
                    results: r,
                }),
            )
            .catch((e) =>
                Object.assign(globalThis.__turBench, {
                    done: true,
                    error: String(e?.stack ?? e),
                }),
            );
        return { started: SCENARIOS };
    }
    return runAll();
})();
