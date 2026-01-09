<script lang="ts">
    let {
        isopen = $bindable(false),
    }: {
        isopen?: boolean;
    } = $props();

    type TabId = "shortcuts" | "dsl" | "about";
    let activeTab = $state<TabId>("shortcuts");

    function handleClose() {
        isopen = false;
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            handleClose();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            handleClose();
        }
    }

    const shortcuts = [
        { keys: ["Ctrl", "S"], action: "Save project (.teis archive)" },
        { keys: ["Ctrl", "O"], action: "Open project or DSL file" },
        { keys: ["Ctrl", "Shift", "Z"], action: "Undo lemmatization" },
        { keys: ["Ctrl", "Shift", "Y"], action: "Redo lemmatization" },
        { keys: ["Ctrl", "Z"], action: "Undo editor change" },
        { keys: ["Ctrl", "Y"], action: "Redo editor change" },
        { keys: ["Ctrl", "F"], action: "Find in editor" },
        { keys: ["F1"], action: "Open help" },
        { keys: ["Ctrl", "/"], action: "Open help (alternative)" },
        { keys: ["Escape"], action: "Close dialog" },
    ];

    const editorShortcuts = [
        { keys: ["Tab"], action: "Indent selection" },
        { keys: ["Shift", "Tab"], action: "Unindent selection" },
        { keys: ["Ctrl", "A"], action: "Select all" },
        { keys: ["Ctrl", "D"], action: "Select word / next occurrence" },
        { keys: ["Ctrl", "/"], action: "Toggle comment" },
        { keys: ["Alt", "↑"], action: "Move line up" },
        { keys: ["Alt", "↓"], action: "Move line down" },
    ];

    const dslSyntax = [
        { syntax: "//", output: "<lb/>", desc: "Line break" },
        {
            syntax: "//5",
            output: '<lb n="5"/>',
            desc: "Line break with number",
        },
        { syntax: "///1v", output: '<pb n="1v"/>', desc: "Page break" },
        {
            syntax: ".abbr[d.]{deus}",
            output: "<choice><abbr>d.</abbr><expan>deus</expan></choice>",
            desc: "Abbreviation",
        },
        {
            syntax: "[...]",
            output: '<gap reason="illegible"/>',
            desc: "Gap/lacuna",
        },
        {
            syntax: "[...3]",
            output: '<gap reason="illegible" quantity="3" unit="chars"/>',
            desc: "Gap with quantity",
        },
        {
            syntax: "[...3<ab>]",
            output: '<gap quantity="3"/><supplied>ab</supplied>',
            desc: "Gap with supplied text",
        },
        {
            syntax: "<text>",
            output: "<supplied>text</supplied>",
            desc: "Supplied text",
        },
        { syntax: "-{text}-", output: "<del>text</del>", desc: "Deletion" },
        { syntax: "+{text}+", output: "<add>text</add>", desc: "Addition" },
        {
            syntax: "^{note}",
            output: "<note>note</note>",
            desc: "Note/annotation",
        },
        {
            syntax: "?{text}?",
            output: "<unclear>text</unclear>",
            desc: "Unclear reading",
        },
        { syntax: ":thorn:", output: "&thorn;", desc: "Entity reference" },
        {
            syntax: "|",
            output: "(word boundary)",
            desc: "Explicit word boundary",
        },
        {
            syntax: "~//",
            output: "(continuation + lb)",
            desc: "Word continues across line",
        },
        {
            syntax: "~///1v",
            output: "(continuation + pb)",
            desc: "Word continues across page",
        },
    ];
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isopen}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="help-title"
        tabindex="-1"
    >
        <div
            class="bg-base-100 rounded-lg shadow-xl w-full max-w-3xl mx-4 max-h-[85vh] flex flex-col"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between p-4 border-b border-base-300"
            >
                <h2 id="help-title" class="text-xl font-bold">Help</h2>
                <button
                    class="btn btn-ghost btn-sm btn-circle"
                    onclick={handleClose}
                    aria-label="Close help"
                >
                    ✕
                </button>
            </div>

            <!-- Tabs -->
            <div class="tabs tabs-bordered px-4 pt-2">
                <button
                    class="tab"
                    class:tab-active={activeTab === "shortcuts"}
                    onclick={() => (activeTab = "shortcuts")}
                >
                    Keyboard Shortcuts
                </button>
                <button
                    class="tab"
                    class:tab-active={activeTab === "dsl"}
                    onclick={() => (activeTab = "dsl")}
                >
                    DSL Syntax
                </button>
                <button
                    class="tab"
                    class:tab-active={activeTab === "about"}
                    onclick={() => (activeTab = "about")}
                >
                    About
                </button>
            </div>

            <!-- Content -->
            <div class="p-4 overflow-y-auto flex-1">
                {#if activeTab === "shortcuts"}
                    <div class="space-y-6">
                        <!-- Application Shortcuts -->
                        <section>
                            <h3 class="text-lg font-semibold mb-3 text-primary">
                                Application
                            </h3>
                            <div class="overflow-x-auto">
                                <table class="table table-sm">
                                    <thead>
                                        <tr>
                                            <th class="w-1/3">Shortcut</th>
                                            <th>Action</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {#each shortcuts as shortcut}
                                            <tr>
                                                <td>
                                                    <div class="flex gap-1">
                                                        {#each shortcut.keys as key, i}
                                                            <kbd
                                                                class="kbd kbd-sm"
                                                                >{key}</kbd
                                                            >
                                                            {#if i < shortcut.keys.length - 1}
                                                                <span
                                                                    class="text-base-content/50"
                                                                    >+</span
                                                                >
                                                            {/if}
                                                        {/each}
                                                    </div>
                                                </td>
                                                <td>{shortcut.action}</td>
                                            </tr>
                                        {/each}
                                    </tbody>
                                </table>
                            </div>
                        </section>

                        <!-- Editor Shortcuts -->
                        <section>
                            <h3 class="text-lg font-semibold mb-3 text-primary">
                                Editor
                            </h3>
                            <div class="overflow-x-auto">
                                <table class="table table-sm">
                                    <thead>
                                        <tr>
                                            <th class="w-1/3">Shortcut</th>
                                            <th>Action</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {#each editorShortcuts as shortcut}
                                            <tr>
                                                <td>
                                                    <div class="flex gap-1">
                                                        {#each shortcut.keys as key, i}
                                                            <kbd
                                                                class="kbd kbd-sm"
                                                                >{key}</kbd
                                                            >
                                                            {#if i < shortcut.keys.length - 1}
                                                                <span
                                                                    class="text-base-content/50"
                                                                    >+</span
                                                                >
                                                            {/if}
                                                        {/each}
                                                    </div>
                                                </td>
                                                <td>{shortcut.action}</td>
                                            </tr>
                                        {/each}
                                    </tbody>
                                </table>
                            </div>
                        </section>

                        <p class="text-sm text-base-content/70 mt-4">
                            <strong>Note:</strong> On macOS, use
                            <kbd class="kbd kbd-xs">Cmd</kbd> instead of
                            <kbd class="kbd kbd-xs">Ctrl</kbd>.
                        </p>
                    </div>
                {:else if activeTab === "dsl"}
                    <div class="space-y-4">
                        <p class="text-sm text-base-content/70">
                            The DSL (Domain-Specific Language) provides
                            shorthand notation for TEI-XML transcription. Type
                            DSL syntax in the editor and it compiles to TEI-XML.
                        </p>

                        <div class="overflow-x-auto">
                            <table class="table table-sm">
                                <thead>
                                    <tr>
                                        <th>DSL Syntax</th>
                                        <th>TEI-XML Output</th>
                                        <th>Description</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {#each dslSyntax as item}
                                        <tr>
                                            <td>
                                                <code
                                                    class="bg-base-200 px-1.5 py-0.5 rounded text-sm font-mono"
                                                    >{item.syntax}</code
                                                >
                                            </td>
                                            <td>
                                                <code
                                                    class="text-xs text-primary font-mono break-all"
                                                    >{item.output}</code
                                                >
                                            </td>
                                            <td class="text-sm">{item.desc}</td>
                                        </tr>
                                    {/each}
                                </tbody>
                            </table>
                        </div>

                        <div class="mt-4 p-3 bg-base-200 rounded-lg">
                            <h4 class="font-semibold mb-2">Entity Browser</h4>
                            <p class="text-sm text-base-content/70">
                                Click the <strong>ꝥ</strong> button in the editor
                                header to browse and insert MENOTA/MUFI character
                                entities. Use the search box to filter by name or
                                description.
                            </p>
                        </div>

                        <div class="p-3 bg-base-200 rounded-lg">
                            <h4 class="font-semibold mb-2">
                                Multi-Level Transcription
                            </h4>
                            <p class="text-sm text-base-content/70">
                                When using the MENOTA template, each word is
                                automatically wrapped with three transcription
                                levels: <code class="text-xs"
                                    >&lt;me:facs&gt;</code
                                >
                                (facsimile),
                                <code class="text-xs">&lt;me:dipl&gt;</code>
                                (diplomatic), and
                                <code class="text-xs">&lt;me:norm&gt;</code>
                                (normalized).
                            </p>
                        </div>
                    </div>
                {:else if activeTab === "about"}
                    <div class="space-y-4">
                        <div class="text-center py-4">
                            <h3 class="text-2xl font-bold">Saga-Scribe</h3>
                            <p class="text-base-content/70">
                                Version 0.1.0 (alpha)
                            </p>
                        </div>

                        <p class="text-sm">
                            A desktop application for scholarly manuscript
                            transcription, producing TEI-XML with MENOTA
                            (Medieval Nordic Text Archive) extensions.
                        </p>

                        <div class="divider"></div>

                        <div class="space-y-2">
                            <h4 class="font-semibold">Features</h4>
                            <ul
                                class="text-sm space-y-1 list-disc list-inside text-base-content/80"
                            >
                                <li>Custom DSL for efficient transcription</li>
                                <li>
                                    Multi-level transcription (facsimile,
                                    diplomatic, normalized)
                                </li>
                                <li>~1,980 MENOTA/MUFI character entities</li>
                                <li>
                                    ONP dictionary integration with
                                    lemmatization
                                </li>
                                <li>
                                    Project archives (.teis) bundling all work
                                </li>
                                <li>Real-time preview as you type</li>
                            </ul>
                        </div>

                        <div class="divider"></div>

                        <div class="space-y-2">
                            <h4 class="font-semibold">License</h4>
                            <p class="text-sm text-base-content/70">
                                GPL-3.0-or-later
                            </p>
                        </div>

                        <div class="space-y-2">
                            <h4 class="font-semibold">Documentation</h4>
                            <p class="text-sm text-base-content/70">
                                See <code>docs/user-guide.md</code> for detailed documentation.
                            </p>
                        </div>

                        <div
                            class="text-center text-sm text-base-content/50 mt-6"
                        >
                            Made with ❤️ in Norway
                        </div>
                    </div>
                {/if}
            </div>

            <!-- Footer -->
            <div
                class="flex items-center justify-end p-4 border-t border-base-300"
            >
                <button class="btn btn-primary btn-sm" onclick={handleClose}>
                    Close
                </button>
            </div>
        </div>
    </div>
{/if}
