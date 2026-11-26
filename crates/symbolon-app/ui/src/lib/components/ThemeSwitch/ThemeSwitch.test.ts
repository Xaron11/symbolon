import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { theme } from '../../design/theme.svelte';
import ThemeSwitch from './ThemeSwitch.svelte';

afterEach(() => theme?.set('system'));

describe('ThemeSwitch', () => {
	it('changes the theme preference', async () => {
		const user = userEvent.setup();
		render(ThemeSwitch);
		const trigger = screen.getByRole('combobox', { name: 'Theme' });
		expect(trigger.textContent).toContain('System');
		trigger.focus();
		await user.keyboard('{Enter}');
		await screen.findByRole('listbox');
		await user.keyboard('{ArrowDown}{Enter}');
		expect(theme?.preference).toBe('dark');
	});
});
