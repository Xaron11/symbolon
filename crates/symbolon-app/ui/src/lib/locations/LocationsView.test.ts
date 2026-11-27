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

	it('scans from the button on each row, then links to the running scan', async () => {
		const user = userEvent.setup();
		const { backend } = view([location('Photos'), location('Stick')]);
		await user.click(await screen.findByRole('button', { name: 'Scan Photos' }));
		await waitFor(() => {
			expect(backend.calls.filter((c) => c.method === 'scans.start')).toEqual([
				{ method: 'scans.start', args: ['Photos'] }
			]);
		});
		backend.emitScan('Photos', { type: 'Started', label: 'Photos', scanId: 1 });
		const link = await screen.findByRole('link', { name: 'Scanning Photos' });
		expect(link.getAttribute('href')).toBe('/scans');
		const busy = screen.getByRole('button', { name: 'Scan Photos' });
		expect(busy.getAttribute('aria-busy')).toBe('true');
		expect(screen.getByRole('button', { name: 'Scan Stick' })).toBeTruthy();
	});

	it('links the last scan to its page', async () => {
		view([
			location('Photos', {
				lastScan: {
					scanId: 3,
					status: 'Completed',
					startedMs: Date.now() - 3 * HOUR,
					finishedMs: Date.now() - 3 * HOUR,
					files: 1,
					bytes: 1,
					warnings: 0,
					error: null
				}
			})
		]);
		const link = await screen.findByRole('link', { name: '3 hours ago' });
		expect(link.getAttribute('href')).toBe('/scans/3');
	});

	it('opens a location from anywhere on its row, but not from its buttons', async () => {
		const user = userEvent.setup({ pointerEventsCheck: 0 });
		const { navigate, backend } = view([location('P')]);
		const row = (await screen.findByRole('link', { name: 'P' })).closest('tr') as HTMLElement;
		await user.click(within(row).getByText('When compared'));
		expect(navigate).toHaveBeenCalledWith('/locations/P');
		navigate.mockClear();
		await user.click(within(row).getByRole('button', { name: 'Scan P' }));
		await waitFor(() => {
			expect(backend.calls.some((c) => c.method === 'scans.start')).toBe(true);
		});
		expect(navigate).not.toHaveBeenCalled();
	});

	it('renames, changes the hash policy and removes from the row menu', async () => {
		const { backend } = view([location('Photos')]);
		let { user, menu } = await openActions('Photos');
		expect(within(menu).queryByRole('menuitem', { name: 'Scan' })).toBeNull();
		await user.keyboard('{Escape}');

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
		const scan = await screen.findByRole('button', { name: 'Scan Old' });
		expect((scan as HTMLButtonElement).disabled).toBe(true);
		expect(screen.getByText('Imported locations cannot be scanned.')).toBeTruthy();
	});

	it('explains scan errors, and opens Scans for a scan already running', async () => {
		const user = userEvent.setup();
		const { backend, navigate } = view([location('Phone'), location('Photos')]);
		backend.scans.start = (label) =>
			Promise.reject(
				label === 'Phone'
					? new ApiError('DeviceNotReady', 'phone A1 is not connected over adb')
					: new ApiError('AlreadyScanning', 'already')
			);
		await user.click(await screen.findByRole('button', { name: 'Scan Phone' }));
		await waitFor(() => {
			expect(toasts.items.at(-1)?.message).toBe(
				'Connect the phone and allow USB debugging, then try again.'
			);
		});
		await user.click(screen.getByRole('button', { name: 'Scan Photos' }));
		await waitFor(() => {
			expect(navigate).toHaveBeenCalledWith('/scans');
		});
	});

	it('opens Add location when asked to', async () => {
		view([], { openAdd: true });
		expect(await screen.findByRole('dialog', { name: 'Add location' })).toBeTruthy();
	});
});

describe('LocationsView add dialog', () => {
	it('starts in the path field and keeps rarely changed options under Advanced', async () => {
		view([], { openAdd: true });
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await waitFor(() => {
			expect(document.activeElement).toBe(within(dialog).getByLabelText('Folder path'));
		});
		const advanced = within(dialog).getByText('Advanced').closest('details');
		expect(advanced?.open).toBe(false);
		expect(advanced?.contains(within(dialog).getByText('Hash algorithm'))).toBe(true);
		expect(
			within(dialog).getByRole('textbox', {
				name: 'Label',
				description: "Defaults to the folder's name."
			})
		).toBeTruthy();
	});

	it('submits once while an add is in flight', async () => {
		const user = userEvent.setup();
		const { backend } = view([location('Old')]);
		let finish: () => void = () => undefined;
		let calls = 0;
		const added = backend.locations.add.bind(backend.locations);
		backend.locations.add = (request) =>
			new Promise((resolve, reject) => {
				calls++;
				finish = () => {
					added(request).then(resolve, reject);
				};
			});
		await user.click(await screen.findByRole('button', { name: 'Add location' }));
		const dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.type(within(dialog).getByLabelText('Folder path'), '/media/photos');
		const add = within(dialog).getByRole('button', { name: 'Add' });
		await user.click(add);
		await user.click(add);
		await user.keyboard('{Enter}');
		finish();
		await waitFor(() => {
			expect(screen.queryByRole('dialog')).toBeNull();
		});
		expect(calls).toBe(1);
	});

	it('tells the page once the palette request was handled', async () => {
		const onaddopened = vi.fn();
		view([], { openAdd: true, onaddopened });
		expect(await screen.findByRole('dialog', { name: 'Add location' })).toBeTruthy();
		expect(onaddopened).toHaveBeenCalledTimes(1);
	});

	it('reads the phones again when asked and each time the dialog opens', async () => {
		// jsdom keeps the dialog's body pointer-events lock for a moment after it closes.
		const user = userEvent.setup({ pointerEventsCheck: 0 });
		const { backend } = view([location('Old')], {}, { devices: [] });
		await user.click(await screen.findByRole('button', { name: 'Add location' }));
		let dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.click(within(dialog).getByRole('tab', { name: 'Android device' }));
		expect(await within(dialog).findByText(/No phones found/)).toBeTruthy();
		backend.seed.devices = [{ serial: 'A1', model: 'Pixel_10', ready: true, hint: null }];
		await user.click(within(dialog).getByRole('button', { name: 'Look again' }));
		expect(await within(dialog).findByRole('radio', { name: /Pixel_10/ })).toBeTruthy();
		await user.keyboard('{Escape}');
		backend.seed.devices = [];
		await user.click(screen.getByRole('button', { name: 'Add location' }));
		dialog = await screen.findByRole('dialog', { name: 'Add location' });
		await user.click(within(dialog).getByRole('tab', { name: 'Android device' }));
		expect(await within(dialog).findByText(/No phones found/)).toBeTruthy();
	});
});
