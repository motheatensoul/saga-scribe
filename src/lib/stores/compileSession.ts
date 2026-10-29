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

interface CompileOutcome {
    revision: number;
    renumbering?: Renumbering | null;
}

class CompileSession {
    private history = new EditHistory();
    /** Revision of the backend's last successful compile */
    private confirmedRevision = 0;
    private queue: Promise<unknown> = Promise.resolve();

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
     */
    run<T extends CompileOutcome>(dsl: string, compile: (guidance?: CompileGuidance) => Promise<T>): Promise<T> {
        // The changes up to this document are cut off now, while the editor holds it
        const request = this.history.begin(dsl.length);
        const execute = async () => {
            const changes = this.history.changesFor(request);
            const guidance = changes ? { baseRevision: this.confirmedRevision, changes } : undefined;
            const result = await compile(guidance);
            // A failed compile (it throws) leaves its changes for the next request
            this.history.confirm(request);
            this.confirmedRevision = result.revision;
            if (result.renumbering) this.onRenumbering?.(result.renumbering);
            return result;
        };
        const done = this.queue.then(execute, execute);
        this.queue = done.catch(() => {});
        return done;
    }
}

export const compileSession = new CompileSession();
