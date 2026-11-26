import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ApiError, type LocationView } from '../api/index.ts';
import { toasts } from '../feedback/toasts.svelte';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import LocationsView from './LocationsView.svelte';

const HOUR = 3_600_000;

function view(locations: LocationView[], extra: Record<string, unknown> = {}, seed = {}) {
	const navigate = vi.fn();
	const rendered = renderWithBackend(LocationsView, { navigate, ...extra }, { locations, ...seed });
	return { ...rendered, navigate };
}

async function openActions(label: string) {
	// jsdom keeps the menu's body pointer-events lock for a moment after it closes.
	const user = userEvent.setup({ pointerEventsCheck: 0 });
	await user.click(await screen.findByRole('button', { name: `Actions for ${label}` }));
	return { user, menu: await screen.findByRole('menu') };
}

describe('LocationsView', () => {
	it('offers to add a location when there is none', async () => {
		const { container } = view([]);
		expect(await screen.findByText('No locations yet')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Add location' })).toBeTruthy();
		await expectNoAxeViolations(container);
	});

	it('lists locations with their status, last scan and contents', async () => {
		const { container } = view([
			location('Photos', {
				lastScan: {
					scanId: 3,
					status: 'Completed',
					startedMs: Date.now() - 3 * HOUR,
					finishedMs: Date.now() - 3 * HOUR,
					files: 1204,
					bytes: 3 * 1024 ** 3,
					warnings: 0,
					error: null
				},
				stats: {
					files: 1204,
					dirs: 10,
					symlinks: 0,
					bytes: 3 * 1024 ** 3,
					verifiedFiles: 0,
					partialFiles: 0
				}
			}),
			location('Stick', { status: { type: 'Missing' } })
		]);
		const photos = (await screen.findByRole('link', { name: 'Photos' })).closest('tr');
		expect(photos?.getAttribute('aria-label') ?? photos?.textContent).toBeTruthy();
		const row = within(photos as HTMLElement);
		expect(row.getByText('Connected')).toBeTruthy();
		expect(row.getByText('3 hours ago')).toBeTruthy();
		expect(row.getByText('1,204')).toBeTruthy();
		expect(row.getByText('3.0 GiB')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Photos' }).getAttribute('href')).toBe(
			'/locations/Photos'
		);
		const stick = within(screen.getByRole('link', { name: 'Stick' }).closest('tr') as HTMLElement);
		expect(stick.getByText('Missing')).toBeTruthy();
		expect(stick.getByText('Never')).toBeTruthy();
		await expectNoAxeViolations(container);
	});

	it('adds a folder chosen with the picker, naming it after the folder', async () => {
		const user = userEvent.setup();
		const { backend } = view([], {}, { pickedFolder: '/home/me/Photos' });
		await user.click(await screen.findByRole('button', { name: 'Add location' }));
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.click(within(dialog).getByRole('button', { name: 'Choose…' }));
		expect(within(dialog).getByLabelText<HTMLInputElement>('Folder path').value).toBe(
			'/home/me/Photos'
		);
		expect(within(dialog).getByLabelText<HTMLInputElement>('Label').value).toBe('Photos');
		await user.click(within(dialog).getByRole('button', { name: 'Add' }));
		await waitFor(() => {
			expect(screen.queryByRole('dialog')).toBeNull();
		});
		expect(backend.calls.find((c) => c.method === 'locations.add')?.args[0]).toEqual({
			label: 'Photos',
			kind: { type: 'Folder', path: '/home/me/Photos' },
			hashPolicy: 'Lazy',
			hashAlgo: null,
			ignorePatterns: []
		});
		expect(await screen.findByRole('link', { name: 'Photos' })).toBeTruthy();
	});

	it('keeps what was typed and marks the label when it is taken', async () => {
		const user = userEvent.setup();
		view([location('Photos')]);
		await user.click(await screen.findByRole('button', { name: 'Add location' }));
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.click(within(dialog).getByRole('button', { name: 'Add' }));
		expect(within(dialog).getByText('Choose a folder.')).toBeTruthy();
		await user.type(within(dialog).getByLabelText('Folder path'), '/media/photos');
		await user.clear(within(dialog).getByLabelText('Label'));
		await user.type(within(dialog).getByLabelText('Label'), 'Photos');
		await user.click(within(dialog).getByRole('button', { name: 'Add' }));
		expect(
			await within(dialog).findByText('A location with this label already exists.')
		).toBeTruthy();
		expect(within(dialog).getByLabelText<HTMLInputElement>('Folder path').value).toBe(
			'/media/photos'
		);
	});

	it('adds a phone from the attached devices, with hints for ones not ready', async () => {
		const user = userEvent.setup();
		const { backend } = view(
			[],
			{},
			{
				devices: [
					{ serial: 'A1', model: 'Pixel_10', ready: true, hint: null },
					{ serial: 'B2', model: null, ready: false, hint: 'Allow USB debugging on the phone.' }
				]
			}
		);
		await user.click(await screen.findByRole('button', { name: 'Add location' }));
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.click(within(dialog).getByRole('tab', { name: 'Android device' }));
		expect(await within(dialog).findByText('Allow USB debugging on the phone.')).toBeTruthy();
		await user.click(within(dialog).getByRole('radio', { name: /Pixel_10/ }));
		await user.type(within(dialog).getByLabelText('Path on the phone'), '/sdcard/DCIM');
		expect(within(dialog).getByLabelText<HTMLInputElement>('Label').value).toBe('DCIM');
		await user.clear(within(dialog).getByLabelText('Label'));
		await user.type(within(dialog).getByLabelText('Label'), 'Phone');
		await user.click(within(dialog).getByRole('button', { name: 'Add' }));
		await waitFor(() => {
			expect(backend.calls.find((c) => c.method === 'locations.add')?.args[0]).toMatchObject({
				label: 'Phone',
				kind: { type: 'Android', serial: 'A1', path: '/sdcard/DCIM' }
			});
		});
	});

	it('scans, renames, changes the hash policy and removes from the row menu', async () => {
		const { backend } = view([location('Photos')]);
		let { user, menu } = await openActions('Photos');
		await user.click(within(menu).getByRole('menuitem', { name: 'Scan' }));
		expect(backend.calls.some((c) => c.method === 'scans.start')).toBe(true);

		({ user, menu } = await openActions('Photos'));
		await user.click(within(menu).getByRole('menuitem', { name: 'Hash during every scan' }));
		await waitFor(() => {
			expect(backend.seed.locations?.[0]?.hashPolicy).toBe('Eager');
		});

		({ user, menu } = await openActions('Photos'));
		await user.click(within(menu).getByRole('menuitem', { name: 'Rename…' }));
		const rename = await screen.findByRole('dialog', { name: 'Rename Photos' });
		await user.clear(within(rename).getByLabelText('New label'));
		await user.type(within(rename).getByLabelText('New label'), 'Pics');
		await user.click(within(rename).getByRole('button', { name: 'Rename' }));
		expect(await screen.findByRole('link', { name: 'Pics' })).toBeTruthy();

		({ user, menu } = await openActions('Pics'));
		await user.click(within(menu).getByRole('menuitem', { name: 'Remove…' }));
		const remove = await screen.findByRole('dialog', { name: 'Remove Pics?' });
		expect(
			within(remove).getByText('Remove Pics from the catalog? Files on disk are not touched.')
		).toBeTruthy();
		await user.click(within(remove).getByRole('button', { name: 'Remove Pics' }));
		expect(await screen.findByText('No locations yet')).toBeTruthy();
	});

	it('points a moved location at its new place', async () => {
		const user = userEvent.setup();
		const { backend } = view([location('Stick', { status: { type: 'Moved', to: '/media/b' } })]);
		await user.click(await screen.findByRole('button', { name: 'Use new path for Stick' }));
		await waitFor(() => {
			expect(backend.calls.some((c) => c.method === 'locations.followMove')).toBe(true);
		});
		expect(await screen.findByText('Connected')).toBeTruthy();
	});

	it('does not scan imported locations', async () => {
		view([location('Old', { kind: 'Imported', status: { type: 'NotScannable' } })]);
		const { menu } = await openActions('Old');
		expect(within(menu).getByRole('menuitem', { name: /Scan/ }).getAttribute('aria-disabled')).toBe(
			'true'
		);
		expect(screen.getByText('Imported locations cannot be scanned.')).toBeTruthy();
	});

	it('explains scan errors, and opens Scans for a scan already running', async () => {
		const { backend, navigate } = view([location('Phone'), location('Photos')]);
		backend.scans.start = (label) =>
			Promise.reject(
				label === 'Phone'
					? new ApiError('DeviceNotReady', 'phone A1 is not connected over adb')
					: new ApiError('AlreadyScanning', 'already')
			);
		let { user, menu } = await openActions('Phone');
		await user.click(within(menu).getByRole('menuitem', { name: 'Scan' }));
		await waitFor(() => {
			expect(toasts.items.at(-1)?.message).toBe(
				'Connect the phone and allow USB debugging, then try again.'
			);
		});
		({ user, menu } = await openActions('Photos'));
		await user.click(within(menu).getByRole('menuitem', { name: 'Scan' }));
		await waitFor(() => {
			expect(navigate).toHaveBeenCalledWith('/scans');
		});
	});

	it('opens Add location when asked to', async () => {
		view([], { openAdd: true });
		expect(await screen.findByRole('dialog', { name: 'Add location' })).toBeTruthy();
	});
});
