/// <reference types="node" />
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// Unit tests for TypeScript modules and Svelte components. Kept separate from
// vite.config.ts so tests don't start the SvelteKit plugin; the Svelte plugin
// compiles components, and the `$lib` alias is all that's needed of the rest.
export default defineConfig({
	plugins: [svelte()],
	resolve: {
		alias: {
			$lib: fileURLToPath(new URL('./src/lib', import.meta.url))
		},
		// Svelte's client runtime, so components can be mounted
		conditions: ['browser']
	},
	test: {
		environment: 'jsdom',
		include: ['src/**/*.test.ts'],
		// The opt-in full-manuscript test parses an 8 MB file repeatedly
		testTimeout: 120_000
	}
});
