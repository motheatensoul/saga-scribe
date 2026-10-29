/**
 * The editor's changes since the DSL the backend last compiled.
 *
 * Imported mode keeps track of which word is which across edits. The backend
 * remembers the DSL of its last successful compile (by revision); sending it
 * the exact changes made since lets words outside the edited ranges keep
 * their identity outright, instead of being matched up by comparison.
 *
 * Changes are grouped into one window per compile request. A request's
 * changes are all unconfirmed windows up to its own; a successful compile
 * confirms them, a failed one leaves them for the next request. Requests
 * must run one at a time, in order.
 */

import { ChangeSet } from "@codemirror/state";
import type { TextChange } from "$lib/tauri";

interface Window {
    id: number;
    /** Changes during this window; null if unknown (e.g. before the first compile) */
    changes: ChangeSet | null;
}

export class EditHistory {
    /** Changes since the last request began */
    private current: ChangeSet | null = null;
    /** Requests not yet confirmed, oldest first */
    private windows: Window[] = [];
    private nextId = 0;

    /** Forgets everything, e.g. when another document is loaded. */
    reset() {
        this.current = null;
        this.windows = [];
    }

    /** Records an editor transaction's changes. */
    record(changes: ChangeSet) {
        if (!this.current) return;
        // Out of step with the document: unknown until the next request
        this.current = this.current.newLength === changes.length ? this.current.compose(changes) : null;
    }

    /**
     * Starts a compile request for the document as it is now (`length` in
     * UTF-16 units, as the editor counts); returns the request's id.
     */
    begin(length: number): number {
        const id = this.nextId++;
        // A document other than the editor's (lengths differ): unknown
        const changes = this.current?.newLength === length ? this.current : null;
        this.windows.push({ id, changes });
        this.current = ChangeSet.empty(length);
        return id;
    }

    /**
     * The changes from the last confirmed compile to request `id`'s document,
     * or null if they aren't fully known.
     */
    changesFor(id: number): TextChange[] | null {
        // Forgotten (another document was loaded since it was made)
        if (!this.windows.some((window) => window.id === id)) return null;
        let composed: ChangeSet | null = null;
        for (const window of this.windows) {
            if (!window.changes) return null;
            if (composed && composed.newLength !== window.changes.length) return null;
            composed = composed ? composed.compose(window.changes) : window.changes;
            if (window.id === id) break;
        }
        if (!composed) return null;

        const changes: TextChange[] = [];
        composed.iterChanges((fromA, toA, fromB, toB) => {
            changes.push({ from_a: fromA, to_a: toA, from_b: fromB, to_b: toB });
        });
        return changes;
    }

    /** Request `id` compiled: its document is the new basis. */
    confirm(id: number) {
        const at = this.windows.findIndex((window) => window.id === id);
        if (at >= 0) this.windows.splice(0, at + 1);
    }
}
