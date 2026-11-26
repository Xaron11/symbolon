import stylelint from 'stylelint';
import { describe, expect, it } from 'vitest';
import config from '../stylelint.config.js';

async function problems(css: string): Promise<number> {
	const { results } = await stylelint.lint({
		code: `<div class="x"></div>\n<style>\n.x { ${css} }\n</style>\n`,
		codeFilename: 'src/lib/components/Fixture/Fixture.svelte',
		config
	});
	return results[0]?.warnings.length ?? 0;
}

describe('style lint (ADR-0022)', () => {
	it.each([
		'color: #fff;',
		'background: #000;',
		'padding: 8px;',
		'row-gap: 5px;',
		'border: 1px solid #fff;',
		'border-top: 2px solid var(--sy-color-border-default);',
		'outline: 3px solid red;',
		'font: 12px Arial;',
		'font-size: 13px;',
		'border-radius: 4px;',
		'box-shadow: 0 1px 2px rgb(0 0 0 / 0.2);',
		'color: rgb(0 0 0);',
		'transition: color 250ms;',
		'transition-duration: 100ms;'
	])('rejects %s', async (css) => {
		expect(await problems(css)).toBeGreaterThan(0);
	});

	it.each([
		'color: var(--sy-color-text-primary);',
		'padding: var(--sy-space-4) 0;',
		'border: var(--sy-border-width-default) solid var(--sy-color-border-default);',
		'margin-inline-start: calc(var(--sy-tree-indent) * 3);',
		'margin-top: calc(var(--sy-space-4) * -1);',
		'background: transparent;',
		'transition: color var(--sy-motion-duration-state) var(--sy-motion-easing-enter);',
		'width: 100%;'
	])('accepts %s', async (css) => {
		expect(await problems(css)).toBe(0);
	});
});
