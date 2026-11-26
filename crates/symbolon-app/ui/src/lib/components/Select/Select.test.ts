import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import SelectHarness from './SelectHarness.test.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('Select', () => {
	it('shows the value and changes it by keyboard', async () => {
		const user = userEvent.setup();
		render(SelectHarness);
		const trigger = screen.getByRole('combobox', { name: 'Hash algorithm' });
		expect(trigger.textContent).toContain('BLAKE3');
		trigger.focus();
		await user.keyboard('{Enter}');
		await expectNoAxeViolations(await screen.findByRole('listbox'));
		await user.keyboard('{ArrowDown}{Enter}');
		expect(screen.getByTestId('value').textContent).toBe('xxh3');
		expect(trigger.textContent).toContain('XXH3');
	});
});
