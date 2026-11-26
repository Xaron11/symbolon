import { Channel } from '@tauri-apps/api/core';
import { commands } from './bindings';
import type { Backend } from './backend';
import { toApiError } from './errors';

/** Awaits `call`, turning a rejection into an `ApiError`. */
async function call<T>(promise: Promise<T>): Promise<T> {
	try {
		return await promise;
	} catch (error) {
		throw toApiError(error);
	}
}

async function done(promise: Promise<null>): Promise<void> {
	await call(promise);
}

function channel<T>(onMessage: (message: T) => void): Channel<T> {
	return new Channel<T>(onMessage);
}

/** The backend of the desktop app: Tauri commands and channels. */
export function createTauriBackend(): Backend {
	return {
		locations: {
			list: () => call(commands.locationsList()),
			add: (request) => call(commands.locationAdd(request)),
			rename: (label, newLabel) => call(commands.locationRename(label, newLabel)),
			remove: (label) => done(commands.locationRemove(label)),
			setHashPolicy: (label, policy) => call(commands.locationSetHashPolicy(label, policy)),
			followMove: (label, choice) => call(commands.locationFollowMove(label, choice ?? null)),
			stats: (label) => call(commands.locationStats(label))
		},
		devices: {
			list: () => call(commands.devicesList())
		},
		scans: {
			start: (label, onUpdate) => done(commands.scanStart(label, channel(onUpdate))),
			attach: (label, onUpdate) => done(commands.scanAttach(label, channel(onUpdate))),
			running: () => call(commands.scansRunning()),
			pause: (label) => done(commands.scanPause(label)),
			resume: (label) => done(commands.scanResume(label)),
			cancel: (label) => done(commands.scanCancel(label)),
			recent: (offset, limit) => call(commands.scansRecent(offset, limit)),
			recentCount: () => call(commands.scansRecentCount()),
			ofLocation: (label, offset, limit) => call(commands.locationScans(label, offset, limit)),
			countOfLocation: (label) => call(commands.locationScanCount(label)),
			warnings: (scanId, offset, limit) => call(commands.scanWarnings(scanId, offset, limit)),
			warningCount: (scanId) => call(commands.scanWarningCount(scanId))
		},
		browse: {
			childCount: (label, parent) => call(commands.browseChildCount(label, parent)),
			children: (label, parent, offset, limit) =>
				call(commands.browseChildren(label, parent, offset, limit)),
			subscribe(label, onChange) {
				// The backend keeps the channel until a send fails; stopping just ignores it.
				let active = true;
				void commands
					.browseSubscribe(
						label,
						channel((change) => {
							if (active) onChange(change);
						})
					)
					.catch(() => undefined);
				return () => {
					active = false;
				};
			}
		},
		compare: {
			open: (request, onUpdate) => call(commands.compareOpen(request, channel(onUpdate))),
			close: (id) => done(commands.compareClose(id)),
			verify: (id) => done(commands.compareVerify(id)),
			cancelVerify: (id) => done(commands.compareCancelVerify(id)),
			summary: (id, filter) => call(commands.compareSummary(id, filter)),
			count: (id, filter) => call(commands.compareCount(id, filter)),
			rows: (id, filter, order, offset, limit) =>
				call(commands.compareRows(id, filter, order, offset, limit)),
			childCount: (id, filter, parent) => call(commands.compareChildCount(id, filter, parent)),
			children: (id, filter, parent, offset, limit) =>
				call(commands.compareChildren(id, filter, parent, offset, limit)),
			byExtension: (id, filter) => call(commands.compareByExtension(id, filter)),
			sizeHistogram: (id, filter) => call(commands.compareSizeHistogram(id, filter)),
			duplicates: (id, side, offset, limit) =>
				call(commands.compareDuplicates(id, side, offset, limit)),
			duplicateTotals: (id) => call(commands.compareDuplicateTotals(id))
		}
	};
}
