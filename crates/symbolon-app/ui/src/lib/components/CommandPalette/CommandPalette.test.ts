import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { isTextEntry } from '../../keyboard/dispatcher';
import CommandPalette from './CommandPalette.svelte';

const commands = () => [
	{ id: 'locations', label: 'Go to Locations', group: 'Navigate', shortcut: 'g l', run: vi.fn() },
	{ id: 'compare', label: 'Go to Compare', group: 'Navigate', shortcut: 'g c', run: vi.fn() },
	{ id: 'dark', label: 'Theme: Dark', group: 'Appearance', run: vi.fn() }
];

describe('CommandPalette', () => {
	it('filters as typed and runs the first match on Enter', async () => {
		const user = userEvent.setup();
		const list = commands();
		const { component } = render(CommandPalette, { open: true, commands: list });
		const input = await screen.findByRole('combobox');
		await waitFor(() => {
			expect(document.activeElement).toBe(input);
		});
		// Not only the field: the whole panel is a text-entry area.
		expect(isTextEntry(screen.getByRole('dialog'))).toBe(true);
		await user.type(input, 'compare');
		expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
			expect.stringContaining('Go to Compare')
		]);
		await expectNoAxeViolations(document.body, ['region']);
		await user.keyboard('{Enter}');
		expect(list[1]?.run).toHaveBeenCalledTimes(1);
		await waitFor(() => {
			expect(screen.queryByRole('dialog')).toBeNull();
		});
		expect(component).toBeTruthy();
	});

	it('says when nothing matches', async () => {
		const user = userEvent.setup();
		render(CommandPalette, { open: true, commands: commands() });
		await user.type(await screen.findByRole('combobox'), 'zzzz');
		expect(screen.getByText('No matching commands')).toBeTruthy();
	});
});
