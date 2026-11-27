<!-- Spec: docs/design/components/scan-card.md -->
<script lang="ts">
	import type { ScanUpdate } from '../api/index.ts';
	import Badge from '../components/Badge/Badge.svelte';
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
	const facts = $derived(
		update.type === 'Progress'
			? [
					['Files', count.format(update.files)],
					['Size', formatBytes(update.bytes)],
					...(update.bytesPerSec > 0 ? [['Speed', `${formatBytes(update.bytesPerSec)}/s`]] : []),
					['Elapsed', formatDuration(update.elapsedMs)],
					...(progress.remaining && !paused ? [['Time left', progress.remaining]] : [])
				]
			: []
	);
</script>

<article class="sy-scan-card" aria-labelledby={headingId}>
	<header class="head">
		<h3 id={headingId}>
			<a href="/locations/{encodeURIComponent(update.label)}">{update.label}</a>
		</h3>
		<Badge tone={paused ? 'warning' : 'accent'}>{paused ? 'Paused' : progress.label}</Badge>
		{#if progress.percent !== undefined}<span class="sy-mono percent">
				{progress.percent}%
			</span>{/if}
	</header>
	<ProgressBar
		label="Scanning {update.label}"
		{...progress.percent !== undefined ? { value: progress.percent / 100 } : {}}
	/>
	{#if facts.length > 0}
		<dl class="facts">
			{#each facts as [name, value] (name)}
				<div>
					<dt>{name}</dt>
					<dd class="sy-mono">{value}</dd>
				</div>
			{/each}
		</dl>
	{/if}
	<div class="actions">
		<!-- One button that changes, so focus stays on it. -->
		<Button size="sm" variant="secondary" onclick={paused ? onresume : onpause}>
			{paused ? 'Resume' : 'Pause'}
		</Button>
		<Button size="sm" variant="secondary" onclick={oncancel}>Cancel</Button>
	</div>
</article>

<style>
	.sy-scan-card {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
		padding: var(--sy-space-16);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
	}

	h3 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-semibold);
	}

	a {
		color: var(--sy-color-text-primary);
		text-decoration: none;
	}

	a:hover {
		text-decoration: underline;
	}

	.percent {
		margin-left: auto;
		color: var(--sy-color-text-secondary);
	}

	.facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--sy-space-8) var(--sy-space-24);
		margin: 0;
	}

	dt {
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	dd {
		margin: 0;
	}

	.actions {
		display: flex;
		gap: var(--sy-space-8);
	}
</style>
