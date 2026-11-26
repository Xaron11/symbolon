import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import ComboboxHarness from './ComboboxHarness.test.svelte';

describe('Combobox', () => {
	it('filters options as typed, highlights the first match and picks with the keyboard', async () => {
		const user = userEvent.setup();
		render(ComboboxHarness);
		const input = screen.getByRole('combobox', { name: 'Location' });
		await user.click(input);
		await user.type(input, 'back');
		const options = await screen.findAllByRole('option');
		expect(options.map((o) => o.textContent.trim())).toEqual(['backup', 'backup-old']);
		expect(options[0]?.hasAttribute('data-highlighted')).toBe(true);
		await user.keyboard('{Enter}');
		expect(screen.getByTestId('value').textContent).toBe('backup');
	});
});
