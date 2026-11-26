import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
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

	it('shows the chosen option in the field, also when set from outside', async () => {
		const user = userEvent.setup();
		render(ComboboxHarness, { initial: 'laptop' });
		const input = screen.getByRole<HTMLInputElement>('combobox', { name: 'Location' });
		expect(input.value).toBe('laptop');
		await user.click(screen.getByRole('button', { name: 'Clear' }));
		expect(input.value).toBe('');
	});

	it('Escape restores the field and clears the filter', async () => {
		const user = userEvent.setup();
		render(ComboboxHarness, { initial: 'phone' });
		const input = screen.getByRole<HTMLInputElement>('combobox', { name: 'Location' });
		await user.click(input);
		await user.clear(input);
		await user.type(input, 'bac');
		await user.keyboard('{Escape}');
		expect(input.value).toBe('phone');
		expect(screen.getByTestId('value').textContent).toBe('phone');
		await user.click(input);
		await user.keyboard('{ArrowDown}');
		expect((await screen.findAllByRole('option')).length).toBe(4);
	});

	it('has no axe violations while open', async () => {
		const user = userEvent.setup();
		render(ComboboxHarness);
		await user.click(screen.getByRole('combobox', { name: 'Location' }));
		await user.keyboard('{ArrowDown}');
		await expectNoAxeViolations(await screen.findByRole('listbox'));
	});
});
