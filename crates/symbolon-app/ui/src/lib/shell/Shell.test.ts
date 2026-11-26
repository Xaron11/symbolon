import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../test/axe';
import { text } from '../../test/snippet';
import { theme } from '../design/theme.svelte';
import Shell from './Shell.svelte';

afterEach(() => {
	theme?.set('system');
	localStorage.clear();
});

function shell(pathname = '/locations') {
	const navigate = vi.fn();
	render(Shell, { pathname, navigate, children: text('View content') });
	return navigate;
}

describe('Shell', () => {
	it('marks the current section and offers a skip link', async () => {
		shell('/compare');
		expect(screen.getByRole('link', { name: /Compare/ }).getAttribute('aria-current')).toBe('page');
		expect(screen.getByRole('link', { name: /Locations/ }).hasAttribute('aria-current')).toBe(
			false
		);
		const skip = screen.getByRole('link', { name: 'Skip to content' });
		expect(skip.getAttribute('href')).toBe('#main');
		expect(document.getElementById('main')?.textContent).toContain('View content');
		await expectNoAxeViolations(document.body);
	});

	it('navigates with g sequences and Ctrl+digits', async () => {
		const user = userEvent.setup();
		const navigate = shell();
		await user.keyboard('gc');
		await user.keyboard('{Control>}2{/Control}');
		expect(navigate.mock.calls).toEqual([['/compare'], ['/scans']]);
	});

	it('toggles the sidebar with Ctrl+B', async () => {
		const user = userEvent.setup();
		shell();
		const nav = screen.getByRole('navigation', { name: 'Sections' });
		expect(nav.dataset['collapsed']).toBeUndefined();
		await user.keyboard('{Control>}b{/Control}');
		expect(nav.dataset['collapsed']).toBe('true');
	});

	it('opens the palette with Ctrl+K and runs a command', async () => {
		const user = userEvent.setup();
		shell();
		await user.keyboard('{Control>}k{/Control}');
		const palette = await screen.findByRole('dialog', { name: 'Command palette' });
		const input = within(palette).getByRole('combobox');
		await waitFor(() => {
			expect(document.activeElement).toBe(input);
		});
		await user.type(input, 'theme dark');
		await user.keyboard('{Enter}');
		expect(theme?.preference).toBe('dark');
	});

	it('opens the shortcut help with ?', async () => {
		const user = userEvent.setup();
		shell();
		await user.keyboard('?');
		expect(await screen.findByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy();
	});
});
