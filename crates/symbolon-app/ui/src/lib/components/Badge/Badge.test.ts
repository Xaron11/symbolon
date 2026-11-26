import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { text } from '../../../test/snippet';
import Badge from './Badge.svelte';

describe('Badge', () => {
	it.each(['neutral', 'accent', 'error', 'warning', 'success', 'info'] as const)(
		'renders the %s tone',
		(tone) => {
			render(Badge, { tone, children: text('12') });
			expect(screen.getByText('12').closest('[data-tone]')?.getAttribute('data-tone')).toBe(tone);
		}
	);

	it('is not a live region', () => {
		const { container } = render(Badge, { children: text('3') });
		expect(container.querySelector('[aria-live]')).toBeNull();
	});
});
