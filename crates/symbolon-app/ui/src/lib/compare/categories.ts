import {
	Check,
	CircleDashed,
	CircleQuestionMark,
	MoveRight,
	EqualNot,
	Pencil,
	TriangleAlert
} from '@lucide/svelte';
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
