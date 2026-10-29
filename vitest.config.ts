/// <reference types="node" />
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Unit tests for plain TypeScript modules. Kept separate from vite.config.ts so
// tests don't start the SvelteKit plugin; only the `$lib` alias is needed.
export default defineConfig({
	resolve: {
		alias: {
			$lib: fileURLToPath(new URL('./src/lib', import.meta.url))
		}
	},
	test: {
		environment: 'jsdom',
		include: ['src/**/*.test.ts'],
		// The opt-in full-manuscript test parses an 8 MB file repeatedly
		testTimeout: 120_000
	}
});
