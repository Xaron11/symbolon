import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createToasts } from '../../feedback/toasts.svelte';
import Toast from './Toast.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('Toast', () => {
	it('announces toasts politely, errors assertively, and dismisses by button', async () => {
		const toasts = createToasts();
		toasts.show('Scan finished', 'success');
		toasts.show('Scan failed', 'error');
		const { container } = render(Toast, { toasts });
		await expectNoAxeViolations(container);
		expect(
			screen.getByText('Scan finished').closest('[aria-live]')?.getAttribute('aria-live')
		).toBe('polite');
		expect(screen.getByText('Scan failed').closest('[aria-live]')?.getAttribute('aria-live')).toBe(
			'assertive'
		);
		const error = screen.getByText('Scan failed').closest('li') as HTMLElement;
		await userEvent.click(within(error).getByRole('button', { name: 'Dismiss' }));
		expect(screen.queryByText('Scan failed')).toBeNull();
	});
});
