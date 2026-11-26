<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import type { ScanUpdate } from '../api/index.ts';
	import { announcement, statusText } from './scans.svelte';

	let { running }: { running: readonly ScanUpdate[] } = $props();
</script>

<a class="sy-scan-status" href="/scans">{statusText(running)}</a>
<span class="sy-visually-hidden" role="status">{announcement(running)}</span>

<style>
	.sy-scan-status {
		color: inherit;
		text-decoration: none;
	}

	.sy-scan-status:hover {
		text-decoration: underline;
	}
</style>
