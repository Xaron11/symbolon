<!-- Renders `component` with a backend and scan state in context, as the root layout does. -->
<script lang="ts" generics="P extends Record<string, unknown>">
	import type { Component } from 'svelte';
	import { provideBackend, type Backend } from '#lib/api/index.ts';
	import { provideScans, type ScanState } from '#lib/scans/scans.svelte.ts';

	interface Props {
		backend: Backend;
		scans: ScanState;
		component: Component<P>;
		viewProps: P;
	}

	let { backend, scans, component: View, viewProps }: Props = $props();
	// svelte-ignore state_referenced_locally
	provideBackend(backend);
	// svelte-ignore state_referenced_locally
	provideScans(scans);
</script>

<View {...viewProps} />
