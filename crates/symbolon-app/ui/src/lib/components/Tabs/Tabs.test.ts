import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import TabsHarness from './TabsHarness.test.svelte';

describe('Tabs', () => {
	it('switches panels with arrow keys', async () => {
		const user = userEvent.setup();
		const { container } = render(TabsHarness);
		expect(screen.getByRole('tabpanel').textContent).toContain('Summary panel');
		screen.getByRole('tab', { name: 'Summary' }).focus();
		await user.keyboard('{ArrowRight}');
		expect(screen.getByRole('tab', { name: 'Files' }).getAttribute('aria-selected')).toBe('true');
		expect(screen.getByRole('tabpanel').textContent).toContain('Files panel');
		await expectNoAxeViolations(container);
	});
});
