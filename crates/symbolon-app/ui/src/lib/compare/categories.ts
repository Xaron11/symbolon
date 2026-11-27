import {
	Check,
	CircleDashed,
	CircleQuestionMark,
	MoveRight,
	EqualNot,
	Pencil,
	TriangleAlert
} from '@lucide/svelte';
import type { CompareRowView, SideRequest } from '../api/index.ts';
import type { IconComponent } from '../components/icon';

/** A comparison category, named as core names it (`Category::name()`). */
export type CategoryName =
	'identical' | 'different' | 'only_in' | 'moved' | 'renamed' | 'collision' | 'unverified';

/** How a category is shown: label, icon and the key of its `category.*` tokens. */
export interface CategoryInfo {
	readonly name: CategoryName;
	readonly label: string;
	readonly tokenKey: string;
	readonly icon: IconComponent;
}

/** Every category, in the order the interface lists them. */
export const CATEGORIES: readonly CategoryInfo[] = [
	{ name: 'identical', label: 'Identical', tokenKey: 'identical', icon: Check },
	{ name: 'different', label: 'Different', tokenKey: 'different', icon: EqualNot },
	{ name: 'only_in', label: 'Only in', tokenKey: 'only-in', icon: CircleDashed },
	{ name: 'moved', label: 'Moved', tokenKey: 'moved', icon: MoveRight },
	{ name: 'renamed', label: 'Renamed', tokenKey: 'renamed', icon: Pencil },
	{ name: 'collision', label: 'Collision', tokenKey: 'collision', icon: TriangleAlert },
	{ name: 'unverified', label: 'Unverified', tokenKey: 'unverified', icon: CircleQuestionMark }
];

/** The display information for `name`. */
export function category(name: CategoryName): CategoryInfo {
	const info = CATEGORIES.find((c) => c.name === name);
	if (!info) throw new Error(`unknown category ${name}`);
	return info;
}

/** The labels of `indices` into `sides`, as words: "Photos", "Photos and Backup". */
export function sideNames(indices: readonly number[], sides: readonly SideRequest[]): string {
	const names = indices.map((i) => sides[i]?.label ?? `side ${String(i + 1)}`);
	const last = names.pop();
	return names.length === 0 ? (last ?? '') : `${names.join(', ')} and ${last ?? ''}`;
}

/** Whether rows of `name` hold one file at different paths: moves and renames. */
export const isMove = (name: CategoryName) => name === 'moved' || name === 'renamed';

/** What a row says, naming the sides it concerns: "Only in Backup", "Moved: Photos → Backup". */
export function rowSummary(row: CompareRowView, sides: readonly SideRequest[]): string {
	const label = category(row.category).label;
	if (row.category === 'only_in') return `${label} ${sideNames(row.presence, sides)}`;
	if (isMove(row.category)) {
		const names = row.presence.map((i) => sides[i]?.label ?? `side ${String(i + 1)}`);
		return `${label}: ${names.join(' → ')}`;
	}
	return label;
}

/** A row's path; for moves and renames each side's path in side order: "old/a → new/a". */
export function rowPath(row: CompareRowView): string {
	if (!isMove(row.category)) return row.display;
	const paths: string[] = [];
	for (const side of row.presence) {
		const path = row.cells.find((c) => c.side === side)?.path;
		if (path !== undefined && !paths.includes(path)) paths.push(path);
	}
	return paths.length > 1 ? paths.join(' → ') : row.display;
}
