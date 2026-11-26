import { GitCompareArrows, HardDrive, ScanSearch } from '@lucide/svelte';
import type { IconComponent } from '../components/icon';
import type { Action } from '../keyboard/keymap';

/** A top-level section of the app. */
export interface Section {
	readonly href: string;
	readonly label: string;
	readonly icon: IconComponent;
	/** The keymap action that opens it. */
	readonly action: Action;
	/** Shown as a hint. */
	readonly shortcut: string;
}

/** The sections, in sidebar order. */
export const SECTIONS: readonly Section[] = [
	{
		href: '/locations',
		label: 'Locations',
		icon: HardDrive,
		action: 'nav.locations',
		shortcut: 'g l'
	},
	{ href: '/scans', label: 'Scans', icon: ScanSearch, action: 'nav.scans', shortcut: 'g s' },
	{
		href: '/compare',
		label: 'Compare',
		icon: GitCompareArrows,
		action: 'nav.compare',
		shortcut: 'g c'
	}
];

/** The section `pathname` belongs to, if any. */
export function sectionOf(pathname: string): Section | undefined {
	return SECTIONS.find((s) => pathname === s.href || pathname.startsWith(`${s.href}/`));
}
