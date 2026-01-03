<script lang="ts">
    let { content = '' }: { content?: string } = $props();
</script>

<div class="preview-container">
    <div class="preview-header">
        <span>TEI-XML Preview</span>
    </div>
    <pre class="preview-content"><code>{content}</code></pre>
</div>

<style>
    .preview-container {
        width: 100%;
        height: 100%;
        display: flex;
        flex-direction: column;
        background: #f6f8fa;
    }

    .preview-header {
        padding: 0.5rem 1rem;
        background: #e1e4e8;
        border-bottom: 1px solid #d1d5da;
        font-weight: 500;
        font-size: 0.875rem;
    }

    .preview-content {
        flex: 1;
        margin: 0;
        padding: 1rem;
        overflow: auto;
        font-family: monospace;
        font-size: 0.875rem;
        line-height: 1.5;
        white-space: pre-wrap;
        word-wrap: break-word;
    }
</style>
