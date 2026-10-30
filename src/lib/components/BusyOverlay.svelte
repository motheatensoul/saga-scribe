<script lang="ts">
    /**
     * Covers the window while long work runs: a spinner, what's happening,
     * and optionally the step it's on and a way to cancel. The spinner is
     * animated by SVG, which keeps turning during heavy work.
     */
    let {
        title,
        step = null,
        oncancel,
    }: {
        title: string;
        step?: string | null;
        oncancel?: () => void;
    } = $props();

    const id = $props.id();
</script>

<div class="modal modal-open">
    <div class="modal-backdrop bg-base-100/50"></div>
    <div
        class="modal-box bg-transparent shadow-none shadow-transparent border-none flex flex-col items-center justify-center overflow-hidden"
        role="alertdialog"
        aria-labelledby="{id}-title"
        aria-describedby={step !== null ? `${id}-step` : undefined}
    >
        <svg viewBox="0 0 50 50" width="48" height="48" aria-hidden="true">
            <circle
                cx="25"
                cy="25"
                r="20"
                fill="none"
                stroke="currentColor"
                stroke-width="4"
                stroke-linecap="round"
                stroke-dasharray="90, 150"
                stroke-dashoffset="0"
                class="text-primary"
            >
                <animateTransform
                    attributeName="transform"
                    type="rotate"
                    from="0 25 25"
                    to="360 25 25"
                    dur="1s"
                    repeatCount="indefinite"
                />
            </circle>
        </svg>
        <p id="{id}-title" class="mt-4 font-bold text-lg text-base-content">
            {title}
        </p>
        {#if step !== null}
            <p id="{id}-step" class="text-sm text-base-content/70" aria-live="polite">
                {step}
            </p>
        {/if}
        {#if oncancel}
            <button class="btn btn-ghost btn-sm mt-4" onclick={oncancel}>
                Cancel
            </button>
        {/if}
    </div>
</div>
