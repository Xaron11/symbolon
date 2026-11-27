<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import { explain } from '../locations/status';
	import Dashboard from './Dashboard.svelte';
	import DrillDown from './DrillDown.svelte';
	import Duplicates from './Duplicates.svelte';
	import { openComparison } from './open.svelte';
	import { fromQuery } from './url';
	import VerifyLine from './VerifyLine.svelte';

	interface Props {
		/** The comparison's URL query (see `url.ts`). */
		query: Pick<URLSearchParams, 'get' | 'has' | 'toString'>;
		/** Opens a route: the picker for a new comparison. */
		navigate: (href: string) => void;
	}

	let { query, navigate }: Props = $props();
	const open = openComparison();
	// svelte-ignore state_referenced_locally
	const request = fromQuery(query);
	// It stays open when people leave, so coming back shows it as they left it.
	const session =
		request instanceof Error
			? undefined
			: // svelte-ignore state_referenced_locally
				open.session(query.toString(), request);

	function startOver() {
		open.close();
		navigate('/compare');
	}
</script>

{#if request instanceof Error}
	<Alert tone="error" title="This link names no comparison">
		{request.message} <a href="/compare">Choose locations</a>
	</Alert>
{:else if session}
	{#if session.error}
		<Alert tone="error" title="Could not compare">
			{explain(session.error)} <a href="/compare">Choose locations</a>
		</Alert>
	{:else if session.summary === undefined}
		<Spinner label="Comparing" />
	{:else}
		<div class="sy-compare">
			<header class="head">
				<h2>
					{session.sides
						.map((s) => (s.subpath ? `${s.label}:${s.subpath}` : s.label))
						.join(' and ')}
				</h2>
				<div class="actions">
					<VerifyLine
						verify={session.verify}
						oncancel={() => void session.cancelVerify()}
						onverify={() => void session.verifyAgain()}
					/>
					<Button size="sm" variant="secondary" onclick={startOver}>New comparison</Button>
				</div>
			</header>
			<Dashboard {session} />
			<DrillDown {session} />
			{#if session.duplicates}<Duplicates {session} />{/if}
		</div>
	{/if}
{/if}

<style>
	.sy-compare {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-12);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--sy-space-12);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-heading);
	}
</style>
