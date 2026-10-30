<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';

	let { children } = $props();

	// The splash (app.html) covers the window until the app is drawn
	onMount(() => {
		const splash = document.getElementById('app-splash');
		if (!splash) return;
		splash.classList.add('done');
		setTimeout(() => splash.remove(), 250);
	});
</script>

<svelte:head>
	<title>Saga Scribe</title>
</svelte:head>

{@render children()}
