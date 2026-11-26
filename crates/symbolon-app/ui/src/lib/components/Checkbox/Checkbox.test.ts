import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import Checkbox from './Checkbox.svelte';

describe('Checkbox', () => {
	it('toggles by click and by Space', async () => {
		const { container } = render(Checkbox, { label: 'Include hidden files' });
		const box = screen.getByRole('checkbox', { name: 'Include hidden files' });
		await userEvent.click(box);
		expect((box as HTMLInputElement).checked).toBe(true);
		await userEvent.keyboard(' ');
		expect((box as HTMLInputElement).checked).toBe(false);
		await expectNoAxeViolations(container);
	});

	it('does not toggle when disabled', async () => {
		render(Checkbox, { label: 'Off', disabled: true });
		const box = screen.getByRole('checkbox');
		await userEvent.click(box);
		expect((box as HTMLInputElement).checked).toBe(false);
	});
});
