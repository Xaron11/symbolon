import {
	type Color,
	differenceCiede2000,
	filterDeficiencyDeuter,
	filterDeficiencyProt,
	oklch,
	parse,
	wcagContrast
} from 'culori';
import { describe, expect, it } from 'vitest';
import { themes, type SemanticToken, type ThemeName } from './generated/tokens';

const THEMES: readonly ThemeName[] = ['light', 'dark'];
// Every surface text, icons and borders can sit on, including row and control states.
const SURFACES = [
	'color.surface.canvas',
	'color.surface.base',
	'color.surface.raised',
	'color.surface.sunken',
	'color.surface.hover',
	'color.surface.active',
	'color.surface.selected'
] as const;
const CATEGORIES = [
	'identical',
	'different',
	'only-in',
	'moved',
	'renamed',
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
		// Chip and row tints carry the label in the normal text colors.
		for (const c of CATEGORIES) {
			pairs.push(['color.text.primary', `category.${c}.bg`]);
			pairs.push(['color.text.secondary', `category.${c}.bg`]);
		}
		const failing = pairs
			.map(([fg, bg]) => ({ fg, bg, ratio: contrast(theme, fg, bg) }))
			.filter(({ ratio }) => ratio < 4.5);
		expect(failing).toEqual([]);
	});

	// Only the accent carries hue; a tinted gray reads as a color (slate looked navy).
	it('surfaces, borders, text and the scrim are pure gray', () => {
		const neutral = tokenNames.filter((name) =>
			/^color\.((surface|border|text)\.|scrim$)/.test(name)
		);
		const tinted = neutral.filter((name) => oklch(color(themes[theme][name])).c > 0.002);
		expect(tinted).toEqual([]);
	});

	// Dark blue fills read as navy next to the neutral surfaces.
	it('blue marks only actions, focus and info text', () => {
		const allowed = /^(color\.accent\.|color\.focus$|color\.text\.on-accent$|status\.info\.fg$)/;
		const blue = tokenNames.filter((name) => {
			const value = themes[theme][name];
			if (!/^(#|rgb)/.test(value)) return false;
			const { c, h } = oklch(color(value));
			return c > 0.02 && h !== undefined && h > 235 && h < 275 && !allowed.test(name);
		});
		expect(blue).toEqual([]);
	});

	it('on-accent text has 4.5:1 on the accent in every state', () => {
		for (const bg of [
			'color.accent.default',
			'color.accent.hover',
			'color.accent.active'
		] as const) {
			expect(contrast(theme, 'color.text.on-accent', bg), bg).toBeGreaterThanOrEqual(4.5);
		}
	});

	it('links (accent text) have 4.5:1 on the surfaces they sit on', () => {
		for (const bg of [
			'color.surface.canvas',
			'color.surface.base',
			'color.surface.raised',
			'status.error.bg',
			'status.warning.bg',
			'status.info.bg'
		] as const) {
			expect(contrast(theme, 'color.accent.default', bg), bg).toBeGreaterThanOrEqual(4.5);
		}
	});

	it('danger buttons have 4.5:1 text', () => {
		expect(contrast(theme, 'color.surface.base', 'status.error.fg')).toBeGreaterThanOrEqual(4.5);
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
