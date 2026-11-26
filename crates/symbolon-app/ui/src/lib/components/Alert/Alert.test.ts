import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { text } from '../../../test/snippet';
import Alert from './Alert.svelte';

describe('Alert', () => {
	it('uses role alert for errors', async () => {
		const { container } = render(Alert, {
			tone: 'error',
			title: 'Scan failed',
			children: text('The drive was removed.')
		});
		const alert = screen.getByRole('alert');
		expect(alert.textContent).toContain('Scan failed');
		expect(alert.textContent).toContain('The drive was removed.');
		await expectNoAxeViolations(container);
	});

	it.each(['warning', 'info', 'success'] as const)('uses role status for %s', (tone) => {
		render(Alert, { tone, title: 'Note' });
		expect(screen.getByRole('status').dataset['tone']).toBe(tone);
	});
});
