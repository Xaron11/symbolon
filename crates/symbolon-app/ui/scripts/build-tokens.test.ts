import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import { buildTokens, DOCS_PATH, sourceGlobs, tokenGroup } from './build-tokens';

let out: string;
let docs: string;

beforeAll(async () => {
	const dir = mkdtempSync(path.join(tmpdir(), 'sy-tokens-'));
	out = path.join(dir, 'generated');
	docs = path.join(dir, 'tokens.md');
	await buildTokens(out, docs);
});

describe('buildTokens', () => {
	it('emits every semantic token for both themes', async () => {
		const module = (await import(pathToFileURL(path.join(out, 'tokens.ts')).href)) as {
			themes: Record<'light' | 'dark', Record<string, string>>;
		};
		const light = Object.keys(module.themes.light);
		expect(Object.keys(module.themes.dark)).toEqual(light);
		for (const name of [
			'color.surface.canvas',
			'color.text.primary',
			'color.focus',
			'category.only-in.fg',
			'category.unverified.border',
			'status.error.fg'
		]) {
			expect(light).toContain(name);
		}
	});

	it('writes dark values under the media query and the data-theme selector', () => {
		const css = readFileSync(path.join(out, 'tokens.css'), 'utf8');
		const block = (selector: string) => {
			const start = css.indexOf(selector);
			expect(start, selector).toBeGreaterThanOrEqual(0);
			return css.slice(start, css.indexOf('}', start));
		};
		expect(block("[data-theme='dark'] {")).toContain('--sy-color-surface-canvas:');
		expect(block(":root:not([data-theme='light'])")).toContain('--sy-color-surface-canvas:');
	});

	it('applies a theme to any element with data-theme, not only the root', () => {
		const css = readFileSync(path.join(out, 'tokens.css'), 'utf8');
		const rule = (selector: RegExp) => {
			const match = selector.exec(css);
			expect(match, String(selector)).not.toBeNull();
			const start = match?.index ?? 0;
			return css.slice(start, css.indexOf('}', start));
		};
		expect(rule(/^\[data-theme='dark'\] \{/m)).toContain('--sy-color-surface-canvas:');
		expect(rule(/^:root,\n\[data-theme='light'\] \{/m)).toContain('--sy-color-surface-canvas:');
	});

	it('never emits primitives', () => {
		const css = readFileSync(path.join(out, 'tokens.css'), 'utf8');
		expect(css).not.toMatch(/--sy-(primitive|neutral|blue)-/);
	});

	it('keeps docs/design/tokens.md current', () => {
		expect(readFileSync(docs, 'utf8')).toBe(readFileSync(DOCS_PATH, 'utf8'));
	});
});

describe('token paths on Windows', () => {
	const dir = 'D:\\a\\symbolon\\design\\tokens';

	it('builds source globs with forward slashes, which globs need', () => {
		expect(sourceGlobs(dir, 'dark', path.win32)).toEqual([
			'D:/a/symbolon/design/tokens/primitive/*.json',
			'D:/a/symbolon/design/tokens/semantic/scale.json',
			'D:/a/symbolon/design/tokens/semantic/dark.json',
			'D:/a/symbolon/design/tokens/component.json'
		]);
	});

	it('groups token files by name, however their path is written', () => {
		expect(tokenGroup('D:\\a\\design\\tokens\\semantic\\scale.json')).toBe('scale');
		expect(tokenGroup('/D:/a/design/tokens/component.json')).toBe('component');
		expect(tokenGroup('D:/a/design/tokens/primitive/color.json')).toBe('primitive');
		expect(tokenGroup('/repo/design/tokens/semantic/dark.json')).toBe('theme');
	});
});
