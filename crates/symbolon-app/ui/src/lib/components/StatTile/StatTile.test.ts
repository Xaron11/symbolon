import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import StatTile from './StatTile.svelte';

describe('StatTile', () => {
	it('shows a label and a monospace value', async () => {
		const { container } = render(StatTile, {
			label: 'Different',
			value: '1,204',
			detail: '3.2 GiB',
			category: 'different'
		});
		expect(screen.getByText('Different')).toBeTruthy();
		expect(screen.getByText('1,204').classList.contains('sy-mono')).toBe(true);
		expect(screen.getByText('3.2 GiB')).toBeTruthy();
		await expectNoAxeViolations(container);
	});
});
