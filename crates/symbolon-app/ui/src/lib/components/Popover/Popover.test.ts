import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { text } from '../../../test/snippet';
import Popover from './Popover.svelte';

describe('Popover', () => {
	it('toggles its content from the trigger and closes on Escape', async () => {
		const user = userEvent.setup();
		render(Popover, { trigger: text('Details'), children: text('Scanned 3 minutes ago') });
		await user.click(screen.getByRole('button', { name: 'Details' }));
		expect(await screen.findByText('Scanned 3 minutes ago')).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByText('Scanned 3 minutes ago')).toBeNull();
	});
});
