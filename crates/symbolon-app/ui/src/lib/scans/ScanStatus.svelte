<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import type { ScanUpdate } from '../api/index.ts';
	import { statusText } from './scans.svelte';

	let { running }: { running: readonly ScanUpdate[] } = $props();
</script>

<a class="sy-scan-status" href="/scans" aria-live="polite">{statusText(running)}</a>

<style>
	.sy-scan-status {
		color: inherit;
		text-decoration: none;
	}

	.sy-scan-status:hover {
		text-decoration: underline;
	}
</style>
