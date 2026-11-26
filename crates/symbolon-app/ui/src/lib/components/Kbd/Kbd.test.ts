import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Kbd from './Kbd.svelte';
import { expectNoAxeViolations } from '../../../test/axe';

describe('Kbd', () => {
	it('renders one key per part', async () => {
		const { container } = render(Kbd, { keys: 'Ctrl+K' });
		await expectNoAxeViolations(container);
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
