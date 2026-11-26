import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { text } from '../../../test/snippet';
import EmptyState from './EmptyState.svelte';

describe('EmptyState', () => {
	it('shows a title, a description and actions', async () => {
		const { container } = render(EmptyState, {
			title: 'No locations yet',
			description: 'Add a folder or a drive to compare.',
			actions: text('Add location')
		});
		expect(screen.getByRole('heading', { name: 'No locations yet' })).toBeTruthy();
		expect(screen.getByText('Add a folder or a drive to compare.')).toBeTruthy();
		expect(screen.getByText('Add location')).toBeTruthy();
		await expectNoAxeViolations(container);
	});
});
