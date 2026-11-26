import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Spinner from './Spinner.svelte';

describe('Spinner', () => {
	it('announces its label', () => {
		render(Spinner);
		expect(screen.getByRole('status', { name: 'Loading' })).toBeTruthy();
	});
});
