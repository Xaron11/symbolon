<!-- Renders `component` with a backend, scan state and open comparison in context, as the root layout does. -->
<script lang="ts" generics="P extends Record<string, unknown>">
	import type { Component } from 'svelte';
	import { provideBackend, type Backend } from '#lib/api/index.ts';
	import { provideOpenComparison, type OpenComparison } from '#lib/compare/open.svelte.ts';
	import { provideScans, type ScanState } from '#lib/scans/scans.svelte.ts';

	interface Props {
		backend: Backend;
		scans: ScanState;
		open: OpenComparison;
		component: Component<P>;
		viewProps: P;
	}

	let { backend, scans, open, component: View, viewProps }: Props = $props();
	// svelte-ignore state_referenced_locally
	provideBackend(backend);
	// svelte-ignore state_referenced_locally
	provideScans(scans);
	// svelte-ignore state_referenced_locally
	provideOpenComparison(open);
</script>

<View {...viewProps} />
