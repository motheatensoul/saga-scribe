<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<title>Saga Scribe</title>
</svelte:head>

{@render children()}
