/**
 * Regenerates the dependency part of THIRD_PARTY_LICENSES.md.
 *
 * Lists every Rust crate compiled into the app (normal dependencies, all
 * platforms; no dev or build dependencies) and every JS package whose code
 * ships in the frontend bundle, with its version and declared licence.
 *
 * Usage: bun run licenses
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..");
const OUTPUT = join(ROOT, "THIRD_PARTY_LICENSES.md");
const BEGIN = "<!-- BEGIN GENERATED DEPENDENCIES -->";
const END = "<!-- END GENERATED DEPENDENCIES -->";

/** Build tools whose runtime code still ends up in the bundle. */
const BUNDLED_DEV_PACKAGES = ["svelte", "@sveltejs/kit", "tailwindcss"];

interface Entry {
    name: string;
    version: string;
    license: string;
    url: string;
}

function rustCrates(): Entry[] {
    const metadata = JSON.parse(
        execFileSync(
            "cargo",
            ["metadata", "--format-version", "1", "--manifest-path", join(ROOT, "src-tauri/Cargo.toml")],
            { maxBuffer: 256 * 1024 * 1024, encoding: "utf8" },
        ),
    );
    const packages = new Map<string, any>(metadata.packages.map((p: any) => [p.id, p]));
    const nodes = new Map<string, any>(metadata.resolve.nodes.map((n: any) => [n.id, n]));
    const root: string = metadata.resolve.root;

    // Normal (non-dev, non-build) dependencies, transitively
    const seen = new Set<string>();
    const queue = [root];
    while (queue.length > 0) {
        const id = queue.pop()!;
        for (const dep of nodes.get(id)?.deps ?? []) {
            const normal = dep.dep_kinds.some((k: any) => k.kind === null);
            if (normal && !seen.has(dep.pkg)) {
                seen.add(dep.pkg);
                queue.push(dep.pkg);
            }
        }
    }
    return [...seen].map((id) => {
        const p = packages.get(id);
        return {
            name: p.name,
            version: p.version,
            license: p.license ?? (p.license_file ? "see license file" : "unknown"),
            url: p.repository ?? `https://crates.io/crates/${p.name}`,
        };
    });
}

function jsPackages(): Entry[] {
    const manifest = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8"));
    const entries = new Map<string, Entry>();
    // [package name, directory it is resolved from]
    const queue: [string, string][] = [
        ...Object.keys(manifest.dependencies ?? {}),
        ...BUNDLED_DEV_PACKAGES,
    ].map((name) => [name, ROOT]);

    while (queue.length > 0) {
        const [name, from] = queue.pop()!;
        const dir = resolvePackage(name, from);
        if (!dir) continue;
        const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
        const key = `${pkg.name}@${pkg.version}`;
        if (entries.has(key)) continue;
        const repository = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
        entries.set(key, {
            name: pkg.name,
            version: pkg.version,
            license: typeof pkg.license === "string" ? pkg.license : (pkg.license?.type ?? "unknown"),
            url: (repository ?? `https://www.npmjs.com/package/${pkg.name}`).replace(/^git\+/, "").replace(/\.git$/, ""),
        });
        for (const dep of Object.keys(pkg.dependencies ?? {})) queue.push([dep, dir]);
    }
    return [...entries.values()];
}

/** Node-style lookup: nearest node_modules/<name> from `from` upwards. */
function resolvePackage(name: string, from: string): string | null {
    let dir = from;
    while (dir.startsWith(ROOT)) {
        const candidate = join(dir, "node_modules", name);
        if (existsSync(join(candidate, "package.json"))) return candidate;
        const parent = join(dir, "..");
        if (parent === dir) break;
        dir = parent;
    }
    return null;
}

function table(entries: Entry[]): string {
    const rows = entries
        .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))
        .map((e) => `| ${e.name} | ${e.version} | ${e.license} | ${e.url} |`);
    return ["| Package | Version | Licence | Source |", "|---|---|---|---|", ...rows].join("\n");
}

function summary(entries: Entry[]): string {
    const counts = new Map<string, number>();
    for (const e of entries) counts.set(e.license, (counts.get(e.license) ?? 0) + 1);
    return [...counts.entries()]
        .sort((a, b) => b[1] - a[1])
        .map(([license, n]) => `${license} (${n})`)
        .join(", ");
}

const rust = rustCrates();
const js = jsPackages();
const generated = [
    BEGIN,
    "",
    "### Rust crates",
    "",
    `${rust.length} crates: ${summary(rust)}.`,
    "",
    table(rust),
    "",
    "### JavaScript packages",
    "",
    `${js.length} packages: ${summary(js)}.`,
    "",
    table(js),
    "",
    END,
].join("\n");

const current = readFileSync(OUTPUT, "utf8");
const start = current.indexOf(BEGIN);
const end = current.indexOf(END);
if (start < 0 || end < 0) {
    throw new Error(`${OUTPUT} has no ${BEGIN} … ${END} section`);
}
writeFileSync(OUTPUT, current.slice(0, start) + generated + current.slice(end + END.length));
console.log(`Listed ${rust.length} Rust crates and ${js.length} JS packages in THIRD_PARTY_LICENSES.md`);
