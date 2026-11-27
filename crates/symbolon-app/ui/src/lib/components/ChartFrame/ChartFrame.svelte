<!-- Spec: docs/design/components/chart-frame.md -->
<script lang="ts" module>
	import {
		BarController,
		BarElement,
		CategoryScale,
		Chart,
		Legend,
		LinearScale,
		Tooltip
	} from 'chart.js';

	// Only what bar charts need, so the rest of Chart.js is left out of the bundle.
	Chart.register(BarController, BarElement, CategoryScale, LinearScale, Tooltip, Legend);
</script>

<script lang="ts">
	import type { ChartConfiguration } from 'chart.js';
	import type { Snippet } from 'svelte';
	import { themes, type SemanticToken } from '../../design/generated/tokens';
	import { theme } from '../../design/theme.svelte';

	interface Props {
		/** Builds the chart from the current theme's token values (a canvas cannot read CSS). */
		config: (palette: Readonly<Record<SemanticToken, string>>) => ChartConfiguration;
		/** Accessible name of the chart. */
		label: string;
		/** The same data as a table: the accessible equivalent. */
		table: Snippet;
		/** Show the table; otherwise only assistive technology reads it. */
		showTable?: boolean;
	}

	let { config, label, table, showTable = false }: Props = $props();
	let canvas = $state<HTMLCanvasElement>();
	const palette = $derived(themes[theme?.resolved ?? 'light']);

	$effect(() => {
		if (!canvas) return;
		const built = config(palette);
		// The frame sets the height, so charts side by side line up.
		built.options = { maintainAspectRatio: false, ...built.options };
		const chart = new Chart(canvas, built);
		return () => {
			chart.destroy();
		};
	});
</script>

<figure class="sy-chart">
	<div class="canvas" role="img" aria-label={label}>
		<canvas bind:this={canvas} aria-hidden="true"></canvas>
	</div>
	<div class:sy-visually-hidden={!showTable}>{@render table()}</div>
</figure>

<style>
	.sy-chart {
		display: grid;
		gap: var(--sy-space-8);
		margin: 0;
	}

	.canvas {
		position: relative;
		height: var(--sy-chart-height);
	}
</style>
