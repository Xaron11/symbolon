import axe from 'axe-core';
import { expect } from 'vitest';

/**
 * Fails the test if axe finds accessibility violations in `container`. Color contrast is always
 * skipped: jsdom cannot compute it, and the token tests cover it.
 */
export async function expectNoAxeViolations(
	container: HTMLElement,
	disabledRules: readonly string[] = []
): Promise<void> {
	const rules = Object.fromEntries(
		['color-contrast', ...disabledRules].map((id) => [id, { enabled: false }])
	);
	const { violations } = await axe.run(container, { rules });
	expect(violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
}
