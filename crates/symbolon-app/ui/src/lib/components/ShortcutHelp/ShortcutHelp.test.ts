import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { KEYMAP } from '../../keyboard/keymap';
import ShortcutHelp from './ShortcutHelp.svelte';

describe('ShortcutHelp', () => {
	it('lists every binding with its keys', async () => {
		render(ShortcutHelp, { open: true });
		const dialog = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
		for (const binding of KEYMAP) {
			expect(dialog.textContent).toContain(binding.description);
		}
		expect(dialog.textContent).toContain('Ctrl');
		expect(screen.getByRole('heading', { name: 'In tables and trees' })).toBeTruthy();
		await expectNoAxeViolations(document.body, ['region']);
	});
});
