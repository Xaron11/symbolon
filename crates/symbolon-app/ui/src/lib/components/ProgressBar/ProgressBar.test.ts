import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import ProgressBar from './ProgressBar.svelte';

describe('ProgressBar', () => {
	it('reports a determinate value in percent', async () => {
		const { container } = render(ProgressBar, { label: 'Scanning', value: 0.42 });
		const bar = screen.getByRole('progressbar', { name: 'Scanning' });
		expect(bar.getAttribute('aria-valuenow')).toBe('42');
		expect(bar.getAttribute('aria-valuemin')).toBe('0');
		expect(bar.getAttribute('aria-valuemax')).toBe('100');
		await expectNoAxeViolations(container);
	});

	it('has no value while indeterminate', () => {
		render(ProgressBar, { label: 'Listing' });
		const bar = screen.getByRole('progressbar', { name: 'Listing' });
		expect(bar.hasAttribute('aria-valuenow')).toBe(false);
		expect(bar.dataset['indeterminate']).toBe('true');
	});

	it('clamps out-of-range values', () => {
		render(ProgressBar, { label: 'Over', value: 1.7 });
		expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100');
	});
});
