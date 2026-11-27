import { screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import ComparePicker from './ComparePicker.svelte';

function picker() {
	const navigate = vi.fn();
	const rendered = renderWithBackend(
		ComparePicker,
		{ navigate },
		{
			locations: [
				location('Photos'),
				location('Backup'),
				location('Phone', { kind: 'Android', hashAlgo: 'sha256' })
			]
		}
	);
	return { ...rendered, navigate };
}

describe('ComparePicker', () => {
	it('needs two locations, or one with duplicates', async () => {
		const user = userEvent.setup();
		const { navigate, container } = picker();
		await user.click(await screen.findByRole('checkbox', { name: 'Photos' }));
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(
			screen.getByText('Choose at least two locations, or one with Find duplicates on.')
		).toBeTruthy();
		expect(navigate).not.toHaveBeenCalled();
		await user.click(
			screen.getByRole('checkbox', { name: 'Find duplicates within each location' })
		);
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(navigate).toHaveBeenCalledWith('/compare/view?sides=Photos&dupes=1');
		await expectNoAxeViolations(container);
	});

	it('opens the comparison with subfolders and options in the URL', async () => {
		const user = userEvent.setup();
		const { navigate } = picker();
		await user.click(await screen.findByRole('checkbox', { name: 'Photos' }));
		await user.click(screen.getByRole('checkbox', { name: 'Backup' }));
		await user.type(screen.getByLabelText('Subfolder of Backup (optional)'), '2024');
		await user.click(screen.getByRole('checkbox', { name: 'Ignore case' }));
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(navigate).toHaveBeenCalledWith('/compare/view?sides=Photos%2CBackup%3A2024&ci=1');
	});

	it('explains locations that use different algorithms', async () => {
		const user = userEvent.setup();
		const { navigate } = picker();
		await user.click(await screen.findByRole('checkbox', { name: 'Photos' }));
		await user.click(screen.getByRole('checkbox', { name: 'Phone' }));
		expect(
			screen.getByText(
				'Photos uses BLAKE3 and Phone uses SHA-256; compare locations with the same algorithm.'
			)
		).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(navigate).not.toHaveBeenCalled();
	});
});

describe('ComparePicker fast mode', () => {
	it('accepts a comma decimal and refuses a negative tolerance', async () => {
		const user = userEvent.setup();
		const { navigate } = picker();
		await user.click(await screen.findByRole('checkbox', { name: 'Photos' }));
		await user.click(screen.getByRole('checkbox', { name: 'Backup' }));
		await user.click(screen.getByRole('checkbox', { name: /Fast mode/ }));
		const tolerance = screen.getByLabelText('Time tolerance in seconds');
		await user.clear(tolerance);
		await user.type(tolerance, '-1');
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(screen.getByText('Enter a tolerance of 0 seconds or more.')).toBeTruthy();
		expect(navigate).not.toHaveBeenCalled();
		await user.clear(tolerance);
		await user.type(tolerance, '1,5');
		await user.click(screen.getByRole('button', { name: 'Compare' }));
		expect(navigate).toHaveBeenCalledWith('/compare/view?sides=Photos%2CBackup&fast=1500');
	});
});
