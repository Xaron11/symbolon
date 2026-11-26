import {
	Archive,
	CircleAlert,
	CircleCheck,
	CircleQuestionMark,
	HardDriveUpload,
	MoveRight,
	Unplug
} from '@lucide/svelte';
import type { ApiError, LocationStatus } from '../api/index.ts';
import type { IconComponent } from '../components/icon';

/** How a location's status is shown, and what can be done about it. */
export interface StatusInfo {
	label: string;
	tone: 'success' | 'warning' | 'error' | 'neutral';
	icon: IconComponent;
	action?: 'followMove' | 'chooseCandidate';
}

/** The badge and action for `status`. */
export function statusInfo(status: LocationStatus): StatusInfo {
	switch (status.type) {
		case 'Connected':
			return { label: 'Connected', tone: 'success', icon: CircleCheck };
		case 'Missing':
			return { label: 'Missing', tone: 'error', icon: CircleAlert };
		case 'Moved':
			return { label: 'Moved', tone: 'warning', icon: MoveRight, action: 'followMove' };
		case 'Ambiguous':
			return {
				label: 'Several places',
				tone: 'warning',
				icon: CircleQuestionMark,
				action: 'chooseCandidate'
			};
		case 'OtherDevice':
			return { label: 'Other device', tone: 'warning', icon: HardDriveUpload };
		case 'DeviceNotAttached':
			return { label: 'Phone not attached', tone: 'warning', icon: Unplug };
		case 'NotScannable':
			return { label: 'Imported', tone: 'neutral', icon: Archive };
	}
}

/** Why a location cannot be scanned, or undefined when it can be. */
export function scanBlocked(status: LocationStatus): string | undefined {
	return status.type === 'NotScannable' ? 'Imported locations cannot be scanned.' : undefined;
}

const SENTENCES: Partial<Record<ApiError['kind'], string>> = {
	DeviceNotReady: 'Connect the phone and allow USB debugging, then try again.',
	NotScannable: 'Imported locations cannot be scanned.',
	AlreadyExists: 'A location with this label already exists.'
};

/** A sentence people can act on for `error` (spec §6); the bridge's message otherwise. */
export function explain(error: ApiError): string {
	return SENTENCES[error.kind] ?? error.message;
}
