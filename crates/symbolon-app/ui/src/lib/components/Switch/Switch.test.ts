import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import Switch from './Switch.svelte';

describe('Switch', () => {
	it('toggles aria-checked by click and by Space', async () => {
		const { container } = render(Switch, { label: 'Hash eagerly' });
		const toggle = screen.getByRole('switch', { name: 'Hash eagerly' });
		expect(toggle.getAttribute('aria-checked')).toBe('false');
		await userEvent.click(toggle);
		expect(toggle.getAttribute('aria-checked')).toBe('true');
		await userEvent.keyboard(' ');
		expect(toggle.getAttribute('aria-checked')).toBe('false');
		await expectNoAxeViolations(container);
	});
});
