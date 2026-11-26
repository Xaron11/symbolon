import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import Input from './Input.svelte';

describe('Input', () => {
	it('has a visible label and takes typed text', async () => {
		const { container } = render(Input, { label: 'Name' });
		const input = screen.getByLabelText('Name');
		await userEvent.type(input, 'photos');
		expect((input as HTMLInputElement).value).toBe('photos');
		expect(screen.getByText('Name').tagName).toBe('LABEL');
		await expectNoAxeViolations(container);
	});

	it('links an error to the field', async () => {
		const { container } = render(Input, { label: 'Label', error: 'Already used' });
		const input = screen.getByLabelText('Label');
		expect(input.getAttribute('aria-invalid')).toBe('true');
		const id = input.getAttribute('aria-describedby') ?? '';
		expect(document.getElementById(id)?.textContent).toBe('Already used');
		await expectNoAxeViolations(container);
	});

	it('can be a search field', () => {
		render(Input, { label: 'Filter', type: 'search' });
		expect(screen.getByRole('searchbox', { name: 'Filter' })).toBeTruthy();
	});
});
