import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Kbd from './Kbd.svelte';

describe('Kbd', () => {
	it('renders one key per part', () => {
		const { container } = render(Kbd, { keys: 'Ctrl+K' });
		const keys = [...container.querySelectorAll('kbd kbd')].map((k) => k.textContent);
		expect(keys).toEqual(['Ctrl', 'K']);
	});

	it('renders sequences as separate keys', () => {
		const { container } = render(Kbd, { keys: 'g l' });
		expect([...container.querySelectorAll('kbd kbd')].map((k) => k.textContent)).toEqual([
			'g',
			'l'
		]);
	});
});
