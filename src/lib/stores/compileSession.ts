/**
 * Compiles, one at a time, each telling the backend what changed since the
 * previous one, so it can keep track of which word is which (see
 * src-tauri/src/identity.rs). Used for native and imported documents alike.
 *
 * The backend reports where the previous compile's words went
 * (`renumbering`); `onRenumbering` moves everything stored by word number
 * along, before the next compile is made, so each compile's annotations are
 * numbered as the backend expects.
 *
 * It also holds the last output: the backend then sends only what changed
 * in it (a few bytes of a document of megabytes), and the session puts the
 * whole document back together for its callers.
 */

import type { ChangeSet } from "@codemirror/state";
import { EditHistory } from "$lib/editor/editHistory";
import type { CompileGuidance, XmlDelta } from "$lib/tauri";
import type { Renumbering } from "$lib/stores/renumber";
import { perfRecord } from "$lib/perf";

interface CompileOutcome {
    revision: number;
    renumbering?: Renumbering | null;
    /** The document whole, or only `delta`, its change to the output held */
    xml?: string | null;
    delta?: XmlDelta | null;
}

/** A compile: `guidance` for the backend, and the revision whose output is held (to send a change to) */
type Compile<T> = (guidance?: CompileGuidance, xmlBase?: number) => Promise<T>;

interface Pending {
    request: number;
    compile: Compile<CompileOutcome>;
    queued: number;
    /** The callers waiting for it: its own, and those of the requests it replaced */
    waiters: { resolve: (result: CompileOutcome) => void; reject: (error: unknown) => void }[];
}

class CompileSession {
    private history = new EditHistory();
    /** Revision of the backend's last successful compile */
    private confirmedRevision = 0;
    private running = false;
    /** The request to compile when the running one is done */
    private pending: Pending | null = null;
    /** The last compile's output, which the backend sends changes to */
    private held: { revision: number; xml: string } | null = null;

    /** Moves annotations and selections along when words were inserted or deleted. */
    onRenumbering: ((map: Renumbering) => void) | null = null;

    /** Records an editor transaction's changes (called for every edit). */
    record(changes: ChangeSet) {
        this.history.record(changes);
    }

    /** Another document was loaded: the next compile starts afresh. */
    reset() {
        this.history.reset();
        this.confirmedRevision = 0;
        this.held = null;
    }

    /**
     * Compiles `dsl` (the editor's content now) after any compile already
     * running. `compile` is called when it's this one's turn, with the
     * changes since the backend's last compile (undefined: start afresh), so
     * it should gather what it sends (annotations) only then.
     *
     * Only the newest request waits: one made while another is still waiting
     * replaces it, and both callers get the newer document's result (the
     * older document is out of date anyway). Compiles all return the same
     * kind of result, so `T` holds for either. The result's `xml` is always
     * the whole document.
     */
    run<T extends CompileOutcome>(dsl: string, compile: Compile<T>): Promise<T & { xml: string }> {
        // The changes up to this document are cut off now, while the editor holds it
        const request = this.history.begin(dsl.length);
        return new Promise<T & { xml: string }>((resolve, reject) => {
            const waiters = this.pending?.waiters ?? [];
            if (this.pending) perfRecord("compile.replaced", 0);
            waiters.push({ resolve: resolve as (result: CompileOutcome) => void, reject });
            this.pending = { request, compile, queued: performance.now(), waiters };
            if (!this.running) void this.drain();
        });
    }

    private async drain() {
        this.running = true;
        while (this.pending) {
            const job = this.pending;
            this.pending = null;
            perfRecord("compile.queue", performance.now() - job.queued);
            try {
                const result = await this.execute(job);
                job.waiters.forEach((waiter) => waiter.resolve(result));
            } catch (error) {
                job.waiters.forEach((waiter) => waiter.reject(error));
            }
        }
        this.running = false;
    }

    private async execute(job: Pending): Promise<CompileOutcome> {
        // Includes the changes of the requests it replaced
        const changes = this.history.changesFor(job.request);
        const guidance = changes ? { baseRevision: this.confirmedRevision, changes } : undefined;
        const held = this.held;
        const result = await job.compile(guidance, held?.revision);
        // The backend sends a change only to the output held (`xmlBase`)
        const delta = result.delta;
        const xml = delta ? held!.xml.slice(0, delta.from) + delta.insert + held!.xml.slice(delta.to) : (result.xml ?? null);
        this.held = xml !== null ? { revision: result.revision, xml } : null;
        // A failed compile (it throws) leaves its changes for the next request
        this.history.confirm(job.request);
        this.confirmedRevision = result.revision;
        if (result.renumbering) this.onRenumbering?.(result.renumbering);
        return { ...result, xml: xml ?? "", delta: null };
    }
}

export const compileSession = new CompileSession();
