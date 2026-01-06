<script lang="ts">
	let { children } = $props();
</script>

<svelte:head>
	<title>TEI Scribe</title>
</svelte:head>

{@render children()}

<style>
	/* Junicode font for MUFI/MENOTA character support */
	@font-face {
		font-family: 'Junicode';
		src: url('/fonts/Junicode.woff2') format('woff2');
		font-weight: normal;
		font-style: normal;
		font-display: swap;
	}

	@font-face {
		font-family: 'Junicode';
		src: url('/fonts/JunicodeVF-Italic.woff2') format('woff2');
		font-weight: normal;
		font-style: italic;
		font-display: swap;
	}

	:global(*) {
		margin: 0;
		padding: 0;
		box-sizing: border-box;
	}

	:global(html, body) {
		height: 100%;
		font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
		background: #ffffff;
		color: #24292e;
	}

	:global(body) {
		overflow: hidden;
	}
</style>
