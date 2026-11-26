import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Spinner from './Spinner.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('Spinner', () => {
	it('announces its label', async () => {
		const { container } = render(Spinner);
		await expectNoAxeViolations(container);
		expect(screen.getByRole('status', { name: 'Loading' })).toBeTruthy();
	});
});
