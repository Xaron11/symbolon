import { render, screen } from '@testing-library/svelte';
import { Settings } from '@lucide/svelte';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import IconButton from './IconButton.svelte';

describe('IconButton', () => {
	it('is named by its label and hides the icon from assistive technology', async () => {
		const { container } = render(IconButton, { label: 'Settings', icon: Settings });
		const button = screen.getByRole('button', { name: 'Settings' });
		expect(button.getAttribute('title')).toBe('Settings');
		expect(button.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
		await expectNoAxeViolations(container);
	});
});
