import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { text } from '../../../test/snippet';
import DropdownMenu from './DropdownMenu.svelte';

describe('DropdownMenu', () => {
	it('opens from the keyboard, skips disabled items and runs the chosen one', async () => {
		const user = userEvent.setup();
		const rescan = vi.fn();
		const remove = vi.fn();
		render(DropdownMenu, {
			label: 'Location actions',
			trigger: text('Actions'),
			items: [
				{ label: 'Rescan', onSelect: rescan, shortcut: 'r' },
				{ label: 'Export', onSelect: vi.fn(), disabled: true },
				{ separator: true },
				{ label: 'Remove', onSelect: remove, danger: true }
			]
		});
		screen.getByRole('button', { name: 'Location actions' }).focus();
		await user.keyboard('{Enter}');
		const menu = await screen.findByRole('menu');
		await expectNoAxeViolations(menu);
		expect(screen.getByRole('menuitem', { name: 'Export' }).getAttribute('aria-disabled')).toBe(
			'true'
		);
		await user.keyboard('{ArrowDown}{Enter}');
		expect(remove).toHaveBeenCalledTimes(1);
		expect(rescan).not.toHaveBeenCalled();
	});
});
