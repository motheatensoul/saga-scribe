/**
 * Checks that the app's version is the same everywhere it is written down.
 *
 * - package.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock,
 *   src-tauri/tauri.conf.json and README.md carry the current version;
 * - CHANGELOG.md's newest entry is the current version, dated or "Unreleased";
 * - the Flatpak/AppStream metadata (flatpak/*.metainfo.xml) lists every
 *   release, newest first, with the date CHANGELOG.md gives it, and the
 *   current version as soon as CHANGELOG.md dates it;
 * - SECURITY.md supports the newest released minor version.
 *
 * Usage: bun run check:versions
 *        bun run check:versions --release v0.7.0   (before publishing a release:
 *        the tag must be the current version, and it must be dated everywhere)
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..");
const METAINFO = "flatpak/app.sagascribe.Sagascribe.metainfo.xml";
const SEMVER = String.raw`\d+\.\d+\.\d+`;
const DATE = /^\d{4}-\d{2}-\d{2}$/;

const read = (path: string) => readFileSync(join(ROOT, path), "utf8");
const errors: string[] = [];
const fail = (message: string) => errors.push(message);

/** The first capture of `pattern` in `path`, or a failure. */
function find(path: string, pattern: RegExp): string | undefined {
    const match = read(path).match(pattern);
    if (!match) fail(`${path}: no version found (expected ${pattern})`);
    return match?.[1];
}

const version = JSON.parse(read("package.json")).version as string;

const sources: [string, string | undefined][] = [
    ["src-tauri/Cargo.toml", find("src-tauri/Cargo.toml", new RegExp(`^\\[package\\][^\\[]*?^version = "(${SEMVER})"`, "m"))],
    ["src-tauri/Cargo.lock", find("src-tauri/Cargo.lock", new RegExp(`name = "saga-scribe"\\nversion = "(${SEMVER})"`))],
    ["src-tauri/tauri.conf.json", JSON.parse(read("src-tauri/tauri.conf.json")).version],
];
const readmeVersions = [...read("README.md").matchAll(new RegExp(`\\bversion\\s+v?(${SEMVER})`, "gi"))].map((m) => m[1]);
if (readmeVersions.length === 0) fail('README.md: no "version X.Y.Z" found');
for (const v of readmeVersions) sources.push(["README.md", v]);

for (const [path, found] of sources) {
    if (found !== undefined && found !== version) {
        fail(`${path}: version ${found}, but package.json says ${version}`);
    }
}

// CHANGELOG.md: "## [X.Y.Z] - YYYY-MM-DD" or "## [X.Y.Z] - Unreleased"
const changelog = new Map<string, string>();
const changelogOrder: string[] = [];
for (const [, v, date] of read("CHANGELOG.md").matchAll(new RegExp(`^## \\[(${SEMVER})\\] - (\\S+)`, "gm"))) {
    changelog.set(v, date);
    changelogOrder.push(v);
}
const newest = changelogOrder[0];
if (newest !== version) {
    fail(`CHANGELOG.md: newest entry is ${newest ?? "missing"}, expected ${version} (add "## [${version}] - Unreleased")`);
}
const versionDate = changelog.get(version);
const released = versionDate !== undefined && DATE.test(versionDate);
if (versionDate !== undefined && !released && versionDate !== "Unreleased") {
    fail(`CHANGELOG.md: ${version} is dated "${versionDate}"; use YYYY-MM-DD or "Unreleased"`);
}

// Flatpak metainfo releases
const metainfo = [...read(METAINFO).matchAll(/<release version="([^"]+)" date="([^"]+)"/g)].map(([, v, date]) => ({ v, date }));
const compare = (a: string, b: string) => {
    const [x, y] = [a, b].map((s) => s.split(".").map(Number));
    return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
};
metainfo.forEach(({ v, date }, i) => {
    if (i > 0 && compare(metainfo[i - 1].v, v) <= 0) fail(`${METAINFO}: releases must be listed newest first (${metainfo[i - 1].v} before ${v})`);
    const expected = changelog.get(v);
    if (expected === undefined) fail(`${METAINFO}: release ${v} is not in CHANGELOG.md`);
    else if (expected !== date) fail(`${METAINFO}: release ${v} is dated ${date}, CHANGELOG.md says ${expected}`);
});
const listed = metainfo.some(({ v }) => v === version);
if (released && !listed) {
    fail(`${METAINFO}: no <release version="${version}" date="${versionDate}"> (CHANGELOG.md dates it)`);
}
if (!released && listed) {
    fail(`${METAINFO}: lists ${version}, which CHANGELOG.md doesn't date yet`);
}

// SECURITY.md supports the newest released minor version
const newestReleased = changelogOrder.find((v) => DATE.test(changelog.get(v) ?? ""));
if (newestReleased) {
    const minor = newestReleased.split(".").slice(0, 2).join(".");
    if (!new RegExp(`^\\|\\s*${minor.replace(".", "\\.")}\\.x\\s*\\|\\s*Yes`, "m").test(read("SECURITY.md"))) {
        fail(`SECURITY.md: the supported-versions table should list ${minor}.x as supported (newest release ${newestReleased})`);
    }
}

// Before a release: the tag is this version, and it is dated everywhere
const releaseFlag = process.argv.indexOf("--release");
if (releaseFlag !== -1) {
    const tag = process.argv[releaseFlag + 1];
    if (tag !== `v${version}`) fail(`release tag ${tag ?? "(missing)"} doesn't match the version: expected v${version}`);
    if (!released) fail(`CHANGELOG.md: set the release date of ${version} before releasing`);
}

if (errors.length > 0) {
    console.error(`Version ${version} is inconsistent:\n${errors.map((e) => `  - ${e}`).join("\n")}`);
    process.exit(1);
}
console.log(`Version ${version} is consistent${released ? ` (released ${versionDate})` : " (unreleased)"}.`);
