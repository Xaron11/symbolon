import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import TooltipHarness from './TooltipHarness.test.svelte';

describe('Tooltip', () => {
	it('describes its trigger when focused', async () => {
		const user = userEvent.setup();
		render(TooltipHarness);
		await user.tab();
		const button = screen.getByRole('button', { name: 'Rescan' });
		expect(document.activeElement).toBe(button);
		const tip = await screen.findByRole('tooltip');
		expect(tip.textContent).toContain('Rescan all locations');
		expect(button.getAttribute('aria-describedby')).toBe(tip.id);
	});
});
