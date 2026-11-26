import {
	type Color,
	differenceCiede2000,
	filterDeficiencyDeuter,
	filterDeficiencyProt,
	parse,
	wcagContrast
} from 'culori';
import { describe, expect, it } from 'vitest';
import { themes, type SemanticToken, type ThemeName } from './generated/tokens';

const THEMES: readonly ThemeName[] = ['light', 'dark'];
const SURFACES = ['color.surface.canvas', 'color.surface.base', 'color.surface.raised'] as const;
const CATEGORIES = [
	'identical',
	'different',
	'only-in',
	'moved',
	'collision',
	'unverified'
] as const;
const STATUSES = ['error', 'warning', 'success', 'info'] as const;

const tokenNames = Object.keys(themes.light) as SemanticToken[];
const textTokens = tokenNames.filter(
	(name) => name.startsWith('color.text.') && name !== 'color.text.on-accent'
);

function contrast(theme: ThemeName, fg: SemanticToken, bg: SemanticToken): number {
	return wcagContrast(themes[theme][fg], themes[theme][bg]);
}

const deltaE = differenceCiede2000();

function color(value: string): Color {
	const parsed = parse(value);
	if (!parsed) throw new Error(`unparsable color ${value}`);
	return parsed;
}

describe.each(THEMES)('%s theme', (theme) => {
	it('text has 4.5:1 on every surface', () => {
		const pairs: [SemanticToken, SemanticToken][] = [];
		for (const bg of SURFACES) {
			for (const fg of textTokens) pairs.push([fg, bg]);
			for (const s of STATUSES) pairs.push([`status.${s}.fg`, bg]);
		}
		for (const s of STATUSES) pairs.push([`status.${s}.fg`, `status.${s}.bg`]);
		const failing = pairs
			.map(([fg, bg]) => ({ fg, bg, ratio: contrast(theme, fg, bg) }))
			.filter(({ ratio }) => ratio < 4.5);
		expect(failing).toEqual([]);
	});

	it('on-accent text has 4.5:1 on the accent', () => {
		expect(contrast(theme, 'color.text.on-accent', 'color.accent.default')).toBeGreaterThanOrEqual(
			4.5
		);
	});

	// Category colors mark icons and tints, never label text, so they need the 3:1 of graphics
	// (WCAG 1.4.11); labels use the text colors.
	it('borders, focus, accent and category graphics have 3:1', () => {
		const graphics: SemanticToken[] = [
			'color.border.strong',
			'color.focus',
			'color.accent.default',
			...CATEGORIES.map((c) => `category.${c}.fg` as const)
		];
		const pairs: [SemanticToken, SemanticToken][] = graphics.flatMap((fg) =>
			SURFACES.map((bg): [SemanticToken, SemanticToken] => [fg, bg])
		);
		for (const c of CATEGORIES) pairs.push([`category.${c}.fg`, `category.${c}.bg`]);
		const failing = pairs
			.map(([fg, bg]) => ({ fg, bg, ratio: contrast(theme, fg, bg) }))
			.filter(({ ratio }) => ratio < 3);
		expect(failing).toEqual([]);
	});

	it('categories are distinct, also with color blindness', () => {
		const colors = CATEGORIES.map((c) => ({ c, color: color(themes[theme][`category.${c}.fg`]) }));
		const simulations: Record<string, (x: Color) => Color> = {
			none: (x) => x,
			protanopia: filterDeficiencyProt(1),
			deuteranopia: filterDeficiencyDeuter(1)
		};
		const failing: string[] = [];
		for (const [name, simulate] of Object.entries(simulations)) {
			const minimum = name === 'none' ? 20 : 10;
			for (const [i, a] of colors.entries()) {
				for (const b of colors.slice(i + 1)) {
					const d = deltaE(simulate(a.color), simulate(b.color));
					if (d < minimum) failing.push(`${name}: ${a.c}/${b.c} ΔE ${d.toFixed(1)}`);
				}
			}
		}
		expect(failing).toEqual([]);
	});
});
