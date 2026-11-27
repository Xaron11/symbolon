<!-- Spec: docs/design/screens/compare-screen.md -->
<script lang="ts">
	import Button from '../components/Button/Button.svelte';
	import type { VerifyState } from './session.svelte';

	interface Props {
		verify: VerifyState;
		oncancel: () => void;
		onverify: () => void;
	}

	let { verify, oncancel, onverify }: Props = $props();
	const count = new Intl.NumberFormat('en-US');

	const text = $derived.by(() => {
		const p = verify.progress;
		switch (verify.state) {
			case 'running':
				if (!p) return 'Verifying';
				return `Verifying ${count.format(p.totalFiles)} files — ${String(
					p.totalBytes > 0 ? Math.min(100, Math.floor((p.bytes / p.totalBytes) * 100)) : 0
				)}%`;
			case 'done':
				return 'All verified';
			case 'cancelled':
				return 'Verification cancelled';
			case 'failed':
				return `Verification failed: ${verify.error ?? ''}`;
		}
	});
</script>

<div class="sy-verify-line">
	<span>{text}</span>
	{#if verify.state === 'running'}
		<Button size="sm" variant="ghost" onclick={oncancel}>Cancel</Button>
	{:else if verify.state !== 'done'}
		<Button size="sm" variant="secondary" onclick={onverify}>Verify</Button>
	{/if}
</div>

<style>
	.sy-verify-line {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
		color: var(--sy-color-text-secondary);
	}
</style>
