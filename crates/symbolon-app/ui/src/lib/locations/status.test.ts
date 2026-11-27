import { describe, expect, it } from 'vitest';
import { ApiError } from '../api/index.ts';
import { explain, statusInfo } from './status';

describe('statusInfo', () => {
	it('names every status with a tone and an action where one exists', () => {
		expect(statusInfo({ type: 'Connected' })).toMatchObject({
			label: 'Connected',
			tone: 'success'
		});
		expect(statusInfo({ type: 'Missing' })).toMatchObject({ label: 'Missing', tone: 'error' });
		expect(statusInfo({ type: 'Moved', to: '/media/b' })).toMatchObject({
			label: 'Moved',
			tone: 'warning',
			action: 'followMove'
		});
		expect(statusInfo({ type: 'Ambiguous', candidates: ['/a', '/b'] })).toMatchObject({
			label: 'Several places',
			action: 'chooseCandidate'
		});
		expect(statusInfo({ type: 'OtherDevice' }).label).toBe('Other device');
		expect(statusInfo({ type: 'DeviceNotAttached' }).label).toBe('Phone not attached');
		expect(statusInfo({ type: 'NotScannable' })).toMatchObject({
			label: 'Imported',
			tone: 'neutral'
		});
	});
});

describe('explain', () => {
	it('turns error kinds into sentences people can act on', () => {
		expect(explain(new ApiError('DeviceNotReady', 'phone ABC is not connected over adb'))).toBe(
			'Connect the phone and allow USB debugging, then try again.'
		);
		expect(explain(new ApiError('NotScannable', 'x'))).toBe(
			'Imported locations cannot be scanned.'
		);
		expect(explain(new ApiError('Io', 'disk full'))).toBe('disk full');
	});
});
