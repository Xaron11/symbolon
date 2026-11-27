<!-- Spec: docs/design/components/scan-card.md -->
<script lang="ts">
	import type { ScanUpdate } from '../api/index.ts';
	import Button from '../components/Button/Button.svelte';
	import ProgressBar from '../components/ProgressBar/ProgressBar.svelte';
	import { formatBytes } from '../format';
	import { formatDuration, progressOf } from './progress';

	interface Props {
		update: ScanUpdate;
		paused: boolean;
		onpause: () => void;
		onresume: () => void;
		oncancel: () => void;
	}

	let { update, paused, onpause, onresume, oncancel }: Props = $props();
	const count = new Intl.NumberFormat('en-US');
	const progress = $derived(progressOf(update));
	// Ids from a label would break on spaces; the heading names the card.
	const headingId = $props.id();
</script>

<article class="sy-scan-card" aria-labelledby={headingId}>
	<header class="head">
		<h3 id={headingId}>
			<a href="/locations/{encodeURIComponent(update.label)}">{update.label}</a>
		</h3>
		<span class="phase">{paused ? 'Paused' : progress.label}</span>
		{#if progress.percent !== undefined}<span class="sy-mono percent">
				{progress.percent}%
			</span>{/if}
	</header>
	<ProgressBar
		label="Scanning {update.label}"
		{...progress.percent !== undefined ? { value: progress.percent / 100 } : {}}
	/>
	{#if update.type === 'Progress'}
		<p class="facts sy-mono">
			<span>{count.format(update.files)} files</span>
			<span>{formatBytes(update.bytes)}</span>
			{#if update.bytesPerSec > 0}<span>{formatBytes(update.bytesPerSec)}/s</span>{/if}
			<span>{formatDuration(update.elapsedMs)}</span>
			{#if progress.remaining && !paused}<span>{progress.remaining}</span>{/if}
		</p>
	{/if}
	<div class="actions">
		<!-- One button that changes, so focus stays on it. -->
		<Button size="sm" variant="secondary" onclick={paused ? onresume : onpause}>
			{paused ? 'Resume' : 'Pause'}
		</Button>
		<Button size="sm" variant="ghost" onclick={oncancel}>Cancel</Button>
	</div>
</article>

<style>
	.sy-scan-card {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-8);
		padding: var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
	}

	.head {
		display: flex;
		align-items: baseline;
		gap: var(--sy-space-12);
	}

	h3 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-medium);
	}

	a {
		color: var(--sy-color-text-primary);
	}

	.phase {
		color: var(--sy-color-text-secondary);
	}

	.percent {
		margin-left: auto;
	}

	.facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--sy-space-16);
		margin: 0;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.actions {
		display: flex;
		gap: var(--sy-space-8);
	}
</style>
