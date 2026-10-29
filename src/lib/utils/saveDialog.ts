/**
 * Save dialog that makes sure the chosen path has the right extension.
 *
 * Native dialogs (GTK in particular) don't append the selected filter's
 * extension when the user types a bare name, so "export" would be written
 * without ".pdf".
 */

import { save, type SaveDialogOptions } from "@tauri-apps/plugin-dialog";

export interface FileType {
    /** Name shown in the dialog's filter, e.g. "PDF" */
    name: string;
    /** Accepted extensions without the dot; the first one is appended when missing */
    extensions: string[];
}

/** Returns `path` with `extensions[0]` appended unless it already ends in one of `extensions`. */
export function withExtension(path: string, extensions: string[]): string {
    const lower = path.toLowerCase();
    if (extensions.length === 0 || extensions.some((ext) => lower.endsWith(`.${ext.toLowerCase()}`))) {
        return path;
    }
    return `${path}.${extensions[0]}`;
}

/** Shows a save dialog for one file type; resolves to the path (with extension), or null if cancelled. */
export async function saveAs(
    type: FileType,
    options: Omit<SaveDialogOptions, "filters"> = {},
): Promise<string | null> {
    const path = await save({ ...options, filters: [type] });
    return path ? withExtension(path, type.extensions) : null;
}
