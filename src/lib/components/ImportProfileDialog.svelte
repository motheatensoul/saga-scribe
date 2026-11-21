<script lang="ts">
    import { dialogFocus } from "$lib/utils/dialogFocus";
    import type { Level, Profile } from "$lib/tauri";
    import { X as CloseButton } from "@lucide/svelte";

    let {
        profile = $bindable(null),
        onconfirm,
    }: {
        /** The profile to confirm; the dialog is open while it is set */
        profile?: Profile | null;
        /** Called with the levels and `<choice>` setting if they were changed */
        onconfirm?: (levels: Level[], choice: boolean) => void;
    } = $props();

    const LEVELS: { level: Level; label: string; element: string }[] = [
        { level: "facs", label: "Facsimile", element: "me:facs" },
        { level: "dipl", label: "Diplomatic", element: "me:dipl" },
        { level: "norm", label: "Normalized", element: "me:norm" },
    ];

    let chosen = $state<Record<Level, boolean>>({ facs: false, dipl: false, norm: false });
    let choice = $state(false);

    // Start from the profile each time the dialog opens
    $effect(() => {
        if (profile) {
            chosen = {
                facs: profile.levels.includes("facs"),
                dipl: profile.levels.includes("dipl"),
                norm: profile.levels.includes("norm"),
            };
            choice = profile.choice;
        }
    });

    let chosenLevels = $derived(LEVELS.filter((l) => chosen[l.level]).map((l) => l.level));

    function share(n: number | undefined): string {
        const total = profile?.counts.tokens ?? 0;
        if (!total || !n) return "not in the file";
        const percent = (100 * n) / total;
        const shown = percent > 0 && percent < 1 ? "<1" : Math.round(percent).toString();
        return `in ${shown}% of words, numerals and punctuation`;
    }

    let hidden = $derived(
        Object.entries(profile?.counts.hidden ?? {})
            .sort((a, b) => b[1] - a[1])
            .slice(0, 15),
    );
    let editions = $derived(
        Object.entries(profile?.counts.lineEditions ?? {}).filter(([ed]) => ed !== ""),
    );

    function close() {
        if (!profile) return;
        const changed =
            chosenLevels.join() !== profile.levels.join() || (chosenLevels.length > 1 && choice) !== profile.choice;
        if (changed) onconfirm?.(chosenLevels, choice);
        profile = null;
    }

    function handleKeydown(e: KeyboardEvent) {
        if (profile && e.key === "Escape") close();
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) close();
    }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if profile}
    <!-- Clicking outside the dialog closes it, like its buttons; not needed for keyboard use -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-profile-title"
        tabindex="-1"
        {@attach dialogFocus}
    >
        <div class="bg-base-100 rounded-lg shadow-xl w-full max-w-xl mx-4 max-h-[90vh] flex flex-col">
            <div class="flex items-center justify-between p-6 border-b border-base-300">
                <h2 id="import-profile-title" class="text-2xl font-bold">Transcription of the imported file</h2>
                <button class="btn btn-ghost btn-sm btn-circle" onclick={close} aria-label="Close and keep these settings">
                    <CloseButton size="16" strokeWidth="3" />
                </button>
            </div>

            <div class="p-6 overflow-y-auto flex-1 space-y-6 text-sm">
                <p>
                    This is how the file encodes its transcription, as read from it. Words you add in the
                    editor are written the same way. Words already in the file are not changed.
                </p>

                <section>
                    <h3 id="import-profile-levels" class="text-lg font-semibold mb-2 text-primary">Levels</h3>
                    <div role="group" aria-labelledby="import-profile-levels" class="space-y-2">
                        {#each LEVELS as { level, label, element } (level)}
                            <label class="flex items-start gap-3 cursor-pointer">
                                <input type="checkbox" class="checkbox checkbox-sm mt-0.5" bind:checked={chosen[level]} />
                                <span>
                                    <span class="font-medium">{label}</span>
                                    <code class="font-mono text-xs">&lt;{element}&gt;</code>
                                    <span class="block text-base-content/80">{share(profile.counts.levels[level])}</span>
                                </span>
                            </label>
                        {/each}
                        <label class="flex items-start gap-3 cursor-pointer pt-2">
                            <input
                                type="checkbox"
                                class="checkbox checkbox-sm mt-0.5"
                                bind:checked={choice}
                                disabled={chosenLevels.length < 2}
                            />
                            <span>
                                <span class="font-medium">Levels in <code class="font-mono text-xs">&lt;choice&gt;</code></span>
                                <span class="block text-base-content/80">
                                    {profile.counts.wordsWithChoice} in the file have them so
                                </span>
                            </span>
                        </label>
                    </div>
                </section>

                <section>
                    <h3 class="text-lg font-semibold mb-2 text-primary">Encoding</h3>
                    <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
                        <dt class="font-medium">Abbreviations</dt>
                        <dd>
                            {#if profile.abbreviations === "am_ex"}
                                <code class="font-mono text-xs">&lt;am&gt;</code> and
                                <code class="font-mono text-xs">&lt;ex&gt;</code>
                            {:else}
                                <code class="font-mono text-xs">&lt;choice&gt;&lt;abbr&gt;&lt;expan&gt;</code>
                            {/if}
                            <span class="text-base-content/80">
                                ({profile.counts.am} am, {profile.counts.ex} ex, {profile.counts.choiceAbbr} abbr)
                            </span>
                        </dd>
                        <dt class="font-medium">Punctuation</dt>
                        <dd>
                            <code class="font-mono text-xs">
                                {profile.punctuation === "pc" ? "<pc>" : "<me:punct>"}
                            </code>
                            <span class="text-base-content/80">({profile.counts.pc} pc, {profile.counts.mePunct} me:punct)</span>
                        </dd>
                        {#if editions.length > 1}
                            <dt class="font-medium">Line breaks</dt>
                            <dd>
                                {editions.map(([ed, n]) => `${ed}: ${n}`).join(", ")}. The shorthand shows the
                                line breaks of all of them as <code class="font-mono text-xs">//n</code>.
                            </dd>
                        {/if}
                    </dl>
                </section>

                {#if hidden.length > 0}
                    <section>
                        <h3 class="text-lg font-semibold mb-2 text-primary">Markup the shorthand doesn't show</h3>
                        <p class="mb-2 text-base-content/80">
                            It is kept as it is, also in words you edit, and can be seen in the XML view.
                        </p>
                        <ul class="grid grid-cols-2 gap-x-4 gap-y-0.5 font-mono text-xs">
                            {#each hidden as [name, n] (name)}
                                <li>&lt;{name}&gt; <span class="text-base-content/80">{n}</span></li>
                            {/each}
                        </ul>
                    </section>
                {/if}
            </div>

            <div class="flex justify-end gap-2 p-6 border-t border-base-300">
                <button class="btn btn-primary" onclick={close}>Confirm</button>
            </div>
        </div>
    </div>
{/if}
