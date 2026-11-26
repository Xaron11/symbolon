import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import DialogHarness from './DialogHarness.test.svelte';

describe('Dialog', () => {
	it('moves focus in, keeps Tab inside, closes on Escape and returns focus', async () => {
		const user = userEvent.setup();
		render(DialogHarness);
		const opener = screen.getByRole('button', { name: 'Add location' });
		await user.click(opener);
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await waitFor(() => {
			expect(dialog.contains(document.activeElement)).toBe(true);
		});
		for (let i = 0; i < 5; i++) {
			await user.tab();
			expect(dialog.contains(document.activeElement)).toBe(true);
		}
		await expectNoAxeViolations(dialog);
		await user.keyboard('{Escape}');
		await waitFor(() => {
			expect(screen.queryByRole('dialog')).toBeNull();
			expect(document.activeElement).toBe(opener);
		});
	});

	it('is described by its description', async () => {
		render(DialogHarness, { open: true });
		const dialog = await screen.findByRole('dialog');
		const id = dialog.getAttribute('aria-describedby') ?? '';
		expect(document.getElementById(id)?.textContent).toBe('A folder, drive or phone.');
	});
});
