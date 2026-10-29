<script lang="ts">
    /**
     * Page for the hidden PDF export window (see src-tauri/src/commands/pdf.rs).
     * It replaces itself with the export document, waits for fonts and layout,
     * then tells the backend to print it to PDF.
     */
    import { onMount } from 'svelte';
    import { printDocumentFailed, printDocumentReady, takePrintDocument } from '$lib/tauri';

    // Timers rather than animation frames: frames may never fire in a hidden window
    const settle = () => new Promise((resolve) => setTimeout(resolve, 50));

    onMount(async () => {
        try {
            const html = await takePrintDocument();
            const exportDoc = new DOMParser().parseFromString(html, 'text/html');
            document.replaceChild(document.adoptNode(exportDoc.documentElement), document.documentElement);

            // Lay out once so the fonts in use start loading, then wait for them
            void document.body.offsetHeight;
            await document.fonts.ready;
            await settle();

            await printDocumentReady();
        } catch (e) {
            await printDocumentFailed(e instanceof Error ? e.message : String(e));
        }
    });
</script>
