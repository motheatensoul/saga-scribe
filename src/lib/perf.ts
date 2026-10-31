/**
 * The frontend's half of the performance log (see src-tauri/src/perf.rs):
 * when the app was started with `--perf-log`, measurements are collected
 * here and sent to the backend, which writes them to the log, once a second.
 * Off, `perfTime` just runs its function and `perfRecord` returns at once.
 *
 * Besides the stages measured explicitly, a frame that comes much later than
 * the display's rate is logged as `ui.stall`: the main thread was busy, with
 * whatever the log shows around it.
 */
import { invoke } from "@tauri-apps/api/core";

type Fields = Record<string, string | number | boolean | null | undefined>;

interface Entry {
    t: number;
    stage: string;
    ms: number;
    fields?: Fields;
}

/** A frame this much later than expected counts as a stall */
const STALL_MS = 50;
const FLUSH_EVERY_MS = 1000;

let enabled = false;
let buffer: Entry[] = [];

/** Milliseconds since the Unix epoch, as the backend logs them */
const epochMs = (at = performance.now()) => performance.timeOrigin + at;

/** Whether measurements are being logged. */
export function perfEnabled(): boolean {
    return enabled;
}

/** Logs that `stage` took `ms` (ending now), with `fields`. */
export function perfRecord(stage: string, ms: number, fields?: Fields) {
    if (!enabled) return;
    buffer.push({ t: epochMs(), stage, ms, fields });
}

/** Runs `work`, logging how long it took as `stage`. */
export async function perfTime<T>(stage: string, work: () => Promise<T> | T, fields?: Fields): Promise<T> {
    if (!enabled) return work();
    const start = performance.now();
    try {
        return await work();
    } finally {
        perfRecord(stage, performance.now() - start, fields);
    }
}

/** Like `perfTime`, for synchronous work. */
export function perfTimeSync<T>(stage: string, work: () => T, fields?: Fields): T {
    if (!enabled) return work();
    const start = performance.now();
    try {
        return work();
    } finally {
        perfRecord(stage, performance.now() - start, fields);
    }
}

function flush() {
    if (buffer.length === 0) return;
    const entries = buffer;
    buffer = [];
    invoke("perf_record", { entries }).catch(() => {});
}

/** Logs frames that come late: the main thread was blocked meanwhile. */
function watchFrames() {
    let last = performance.now();
    let interval = 1000 / 60;
    const frame = (now: number) => {
        const gap = now - last;
        // The display's rate, from the shortest recent gaps
        if (gap > 0 && gap < interval) interval = gap;
        // Hidden windows get no frames; a gap from that isn't a stall
        if (gap - interval > STALL_MS && document.visibilityState === "visible") {
            buffer.push({ t: epochMs(now), stage: "ui.stall", ms: gap - interval });
        }
        last = now;
        requestAnimationFrame(frame);
    };
    document.addEventListener("visibilitychange", () => (last = performance.now()));
    requestAnimationFrame(frame);
}

/**
 * Asks the backend whether the log is on, and if so starts measuring.
 * Returns the log's path.
 */
export async function initPerf(): Promise<string | null> {
    const path = await invoke<string | null>("perf_log_path").catch(() => null);
    if (!path) return null;
    enabled = true;
    setInterval(flush, FLUSH_EVERY_MS);
    window.addEventListener("beforeunload", flush);
    watchFrames();
    return path;
}
