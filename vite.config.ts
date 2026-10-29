import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			preprocess: vitePreprocess(),
			adapter: adapter({
				fallback: 'index.html'
			}),
			// Keep the project-wide `$lib/` import convention (SvelteKit 3 dropped the built-in alias)
			alias: {
				$lib: 'src/lib'
			}
		}),
		tailwindcss()
	],
	server: {
		watch: {
			// Build output, not sources: the Rust target directory, and local
			// Flatpak builds (a whole sandbox filesystem, with symlink loops)
			ignored: ['**/src-tauri/target/**', '**/flatpak-build/**', '**/flatpak-repo/**']
		}
	},
	resolve: {
		dedupe: [
			'@codemirror/state',
			'@codemirror/view',
			'@codemirror/language',
			'@codemirror/commands',
			'@codemirror/search',
			'@codemirror/lint',
			'@codemirror/lang-xml'
		]
	}
});
