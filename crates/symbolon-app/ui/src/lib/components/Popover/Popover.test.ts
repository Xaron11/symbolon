import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { text } from '../../../test/snippet';
import Popover from './Popover.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('Popover', () => {
	it('toggles its content from the trigger and closes on Escape', async () => {
		const user = userEvent.setup();
		render(Popover, {
			label: 'Scan details',
			trigger: text('Details'),
			children: text('Scanned 3 minutes ago')
		});
		await user.click(screen.getByRole('button', { name: 'Details' }));
		const content = await screen.findByText('Scanned 3 minutes ago');
		expect(screen.getByRole('dialog', { name: 'Scan details' }).contains(content)).toBe(true);
		// The test page has no landmarks; the app shell provides them.
		await expectNoAxeViolations(document.body, ['region']);
		expect(content).toBeTruthy();
		await user.keyboard('{Escape}');
		await waitFor(() => {
			expect(screen.queryByText('Scanned 3 minutes ago')).toBeNull();
		});
	});
});
