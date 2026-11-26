<!-- Spec: docs/design/components/progress-bar.md -->
<script lang="ts">
	interface Props {
		/** Accessible name, e.g. "Scanning photos". */
		label: string;
		/** Fraction done, 0–1; undefined while the total is unknown. */
		value?: number;
		class?: string;
	}

	let { label, value, class: className }: Props = $props();
	const percent = $derived(
		value === undefined ? undefined : Math.round(Math.min(1, Math.max(0, value)) * 100)
	);
</script>

<div
	class={['sy-progress', className]}
	role="progressbar"
	aria-label={label}
	aria-valuemin="0"
	aria-valuemax="100"
	aria-valuenow={percent}
	data-indeterminate={percent === undefined || undefined}
>
	<div class="fill" style:width={percent === undefined ? undefined : `${String(percent)}%`}></div>
</div>

<style>
	.sy-progress {
		overflow: hidden;
		height: var(--sy-space-4);
		border-radius: var(--sy-radius-full);
		background: var(--sy-color-border-default);
	}

	.fill {
		height: 100%;
		border-radius: inherit;
		background: var(--sy-color-accent-default);
		transition: width var(--sy-motion-duration-enter) var(--sy-motion-easing-enter);
	}

	[data-indeterminate] .fill {
		width: 30%;
		animation: slide calc(var(--sy-motion-duration-enter) * 8) var(--sy-motion-easing-enter)
			infinite;
	}

	@keyframes slide {
		from {
			transform: translateX(-100%);
		}
		to {
			transform: translateX(340%);
		}
	}
</style>
