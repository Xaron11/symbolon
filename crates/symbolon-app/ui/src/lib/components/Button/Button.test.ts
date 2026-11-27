import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { text } from '../../../test/snippet';
import Button from './Button.svelte';

describe('Button', () => {
	it('renders its label as a secondary button by default', async () => {
		const { container } = render(Button, { children: text('Scan') });
		const button = screen.getByRole('button', { name: 'Scan' });
		expect(button.getAttribute('type')).toBe('button');
		expect(button.dataset['variant']).toBe('secondary');
		expect(button.dataset['size']).toBe('md');
		await expectNoAxeViolations(container);
	});

	it.each(['primary', 'secondary', 'ghost', 'danger'] as const)(
		'sets the %s variant',
		(variant) => {
			render(Button, { variant, children: text('Go') });
			expect(screen.getByRole('button').dataset['variant']).toBe(variant);
		}
	);

	it('calls onclick, but not when disabled', async () => {
		const onclick = vi.fn();
		const { rerender } = render(Button, { onclick, children: text('Go') });
		await userEvent.click(screen.getByRole('button'));
		expect(onclick).toHaveBeenCalledTimes(1);
		await rerender({ onclick, disabled: true, children: text('Go') });
		await userEvent.click(screen.getByRole('button'));
		expect(onclick).toHaveBeenCalledTimes(1);
	});

	it('keeps keyboard focus when loading starts', async () => {
		const { rerender } = render(Button, { children: text('Save') });
		const button = screen.getByRole('button', { name: 'Save' });
		button.focus();
		await rerender({ loading: true, children: text('Save') });
		expect(document.activeElement).toBe(button);
		expect(button.getAttribute('aria-disabled')).toBe('true');
	});

	it('is busy and inert while loading', async () => {
		const onclick = vi.fn();
		render(Button, { onclick, loading: true, children: text('Save') });
		const button = screen.getByRole('button', { name: 'Save' });
		expect(button.getAttribute('aria-busy')).toBe('true');
		await userEvent.click(button);
		expect(onclick).not.toHaveBeenCalled();
	});
});

describe('Button as a link', () => {
	it('renders a link styled as a button when it goes somewhere', () => {
		render(Button, {
			href: '/locations/Photos',
			variant: 'secondary',
			children: text('Open Photos')
		});
		const link = screen.getByRole('link', { name: 'Open Photos' });
		expect(link.getAttribute('href')).toBe('/locations/Photos');
		expect(link.classList.contains('sy-button')).toBe(true);
		expect(screen.queryByRole('button')).toBeNull();
	});
});
