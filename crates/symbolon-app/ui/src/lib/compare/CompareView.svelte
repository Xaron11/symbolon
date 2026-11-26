<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import { backend } from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import { explain } from '../locations/status';
	import Dashboard from './Dashboard.svelte';
	import { createCompareSession } from './session.svelte';
	import { fromQuery } from './url';
	import VerifyLine from './VerifyLine.svelte';

	interface Props {
		/** The comparison's URL query (see `url.ts`). */
		query: Pick<URLSearchParams, 'get' | 'has'>;
	}

	let { query }: Props = $props();
	const api = backend();
	// svelte-ignore state_referenced_locally
	const request = fromQuery(query);
	const session = request instanceof Error ? undefined : createCompareSession(api, request);

	$effect(() => {
		if (!session) return;
		void session.open();
		return () => {
			session.close();
		};
	});
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
				<VerifyLine
					verify={session.verify}
					oncancel={() => void session.cancelVerify()}
					onverify={() => void session.verifyAgain()}
				/>
			</header>
			<Dashboard {session} />
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

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-heading);
	}
</style>
