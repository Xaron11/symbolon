// Fixtures for screens rendered against the memory backend.
import { render } from '@testing-library/svelte';
import type { Component } from 'svelte';
import {
	createMemoryBackend,
	type LocationView,
	type MemoryBackend,
	type MemorySeed
} from '#lib/api/index.ts';
import { createScanState } from '#lib/scans/scans.svelte.ts';
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
	prepare?: (backend: MemoryBackend) => void
) {
	const backend = createMemoryBackend(seed);
	prepare?.(backend);
	const scans = createScanState(backend);
	const result = render(WithBackend<P>, {
		props: { backend, scans, component, viewProps: props }
	});
	return { ...result, backend, scans };
}
