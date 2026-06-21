import { createSvgResource } from "@tur/edgy";

// Inline SVG icons registered once at module load. Vello rasterises them up
// front via `createSvgResource` (the same path the todolist icons use). Keep
// the SVGs minimal: explicit width/height/viewBox, no currentColor, basic
// shapes only — usvg/resvg handles these reliably.

const RUN_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#ffffff"><polygon points="6 4 20 12 6 20"/></svg>`;

const RESET_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/></svg>`;

const CUT_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><line x1="20" y1="4" x2="8.12" y2="15.88"/><line x1="14.47" y1="14.48" x2="20" y2="20"/><line x1="8.12" y1="8.12" x2="12" y2="12"/></svg>`;

const COPY_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;

const PASTE_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>`;

const PLAY_SVG = RUN_SVG;
const PAUSE_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#ffffff"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>`;
const STOP_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#ffffff"><rect x="5" y="5" width="14" height="14"/></svg>`;
const REVERSE_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#ffffff"><polygon points="18 4 4 12 18 20"/></svg>`;

export const runIconId: number = createSvgResource(RUN_SVG);
export const resetIconId: number = createSvgResource(RESET_SVG);
export const cutIconId: number = createSvgResource(CUT_SVG);
export const copyIconId: number = createSvgResource(COPY_SVG);
export const pasteIconId: number = createSvgResource(PASTE_SVG);
export const playIconId: number = createSvgResource(PLAY_SVG);
export const pauseIconId: number = createSvgResource(PAUSE_SVG);
export const stopIconId: number = createSvgResource(STOP_SVG);
export const reverseIconId: number = createSvgResource(REVERSE_SVG);
