import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { DOCS_PATH, keymapMarkdown } from './build-keymap-doc';

describe('keyboard page', () => {
	it('is current', async () => {
		expect(await keymapMarkdown()).toBe(readFileSync(DOCS_PATH, 'utf8'));
	});
});
