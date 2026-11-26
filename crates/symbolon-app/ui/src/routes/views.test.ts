import { render } from '@testing-library/svelte';
import type { Component } from 'svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../test/axe';
import Compare from './compare/+page.svelte';
import Scans from './scans/+page.svelte';

describe.each([
	['Scans', Scans],
	['Compare', Compare]
] as [string, Component][])('%s view', (name, View) => {
	it('shows its placeholder accessibly', async () => {
		const { container } = render(View);
		expect(container.textContent).toContain(name);
		await expectNoAxeViolations(container);
	});
});
