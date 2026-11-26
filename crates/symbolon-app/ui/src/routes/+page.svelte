<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';

	const version = invoke<string>('core_version');
</script>

<main>
	<h1>Symbolon</h1>
	<p>
		{#await version then v}
			core {v}
		{:catch}
			core unavailable (running outside Tauri?)
		{/await}
	</p>
</main>

<style>
	main {
		padding: var(--sy-space-32);
	}

	p {
		color: var(--sy-color-text-muted);
	}
</style>
