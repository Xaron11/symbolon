import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { text } from '../../../test/snippet';
import ContextMenu from './ContextMenu.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('ContextMenu', () => {
	it('opens on right click over its target', async () => {
		const open = vi.fn();
		render(ContextMenu, {
			items: [{ label: 'Open', onSelect: open }],
			children: text('photo.jpg')
		});
		await fireEvent.contextMenu(screen.getByText('photo.jpg'));
		await expectNoAxeViolations(await screen.findByRole('menu'));
		await userEvent.click(await screen.findByRole('menuitem', { name: 'Open' }));
		expect(open).toHaveBeenCalledTimes(1);
	});
});
