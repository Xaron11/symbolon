// Fixtures for screens rendered against the memory backend.
import { render } from '@testing-library/svelte';
import type { Component } from 'svelte';
import {
	createMemoryBackend,
	type LocationView,
	type MemoryBackend,
	type MemorySeed
} from '#lib/api/index.ts';
import { createOpenComparison, type OpenComparison } from '#lib/compare/open.svelte.ts';
import { createScanState, type ScanState } from '#lib/scans/scans.svelte.ts';
import WithBackend from './WithBackend.svelte';

/** A location as the bridge reports it, with `overrides`. */
export function location(label: string, overrides: Partial<LocationView> = {}): LocationView {
	return {
		label,
		root: `/media/${label.toLowerCase()}`,
		kind: 'Folder',
		status: { type: 'Connected' },
		hashPolicy: 'Lazy',
		hashAlgo: 'blake3',
		lastScan: null,
		stats: { files: 0, dirs: 0, symlinks: 0, bytes: 0, verifiedFiles: 0, partialFiles: 0 },
		...overrides
	};
}

/** Renders `component` over a memory backend seeded with `seed`, changed by `prepare` first. */
export function renderWithBackend<P extends Record<string, unknown>>(
	component: Component<P>,
	props: P,
	seed: MemorySeed = {},
	prepare?: (backend: MemoryBackend, open: OpenComparison) => void
) {
	const backend = createMemoryBackend(seed);
	const open = createOpenComparison(backend);
	prepare?.(backend, open);
	const scans = createScanState(backend);
	const result = render(WithBackend<P>, {
		props: { backend, scans, open, component, viewProps: props }
	});
	return { ...result, backend, scans, open };
}

/** Renders `component` again over the same backend and app state, as after navigating back. */
export function renderAgain<P extends Record<string, unknown>>(
	component: Component<P>,
	props: P,
	context: { backend: MemoryBackend; scans: ScanState; open: OpenComparison }
) {
	const { backend, scans, open } = context;
	return render(WithBackend<P>, {
		props: { backend, scans, open, component, viewProps: props }
	});
}
