/**
 * Compiles, one at a time, each telling the backend what changed since the
 * previous one, so it can keep track of which word is which (see
 * src-tauri/src/identity.rs). Used for native and imported documents alike.
 *
 * The backend reports where the previous compile's words went
 * (`renumbering`); `onRenumbering` moves everything stored by word number
 * along, before the next compile is made, so each compile's annotations are
 * numbered as the backend expects.
 */

import type { ChangeSet } from "@codemirror/state";
import { EditHistory } from "$lib/editor/editHistory";
import type { CompileGuidance } from "$lib/tauri";
import type { Renumbering } from "$lib/stores/renumber";
import { perfRecord } from "$lib/perf";

interface CompileOutcome {
    revision: number;
    renumbering?: Renumbering | null;
}

interface Pending {
    request: number;
    compile: (guidance?: CompileGuidance) => Promise<CompileOutcome>;
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
     * kind of result, so `T` holds for either.
     */
    run<T extends CompileOutcome>(dsl: string, compile: (guidance?: CompileGuidance) => Promise<T>): Promise<T> {
        // The changes up to this document are cut off now, while the editor holds it
        const request = this.history.begin(dsl.length);
        return new Promise<T>((resolve, reject) => {
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
        const result = await job.compile(guidance);
        // A failed compile (it throws) leaves its changes for the next request
        this.history.confirm(job.request);
        this.confirmedRevision = result.revision;
        if (result.renumbering) this.onRenumbering?.(result.renumbering);
        return result;
    }
}

export const compileSession = new CompileSession();
