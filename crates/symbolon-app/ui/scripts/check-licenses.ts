// Fails if a production dependency is under a license that would keep the app from being
// distributed under MIT OR Apache-2.0.
// Mirrors the allowlist in the repository's deny.toml.
import { execFileSync } from 'node:child_process';

const ALLOWED = new Set([
	'0BSD',
	'Apache-2.0',
	'BSD-2-Clause',
	'BSD-3-Clause',
	'BSL-1.0',
	'CC0-1.0',
	'ISC',
	'MIT',
	'MPL-2.0',
	'Unicode-3.0',
	'Zlib',
	// Fonts only (ADR-0022).
	'OFL-1.1'
]);

interface PackageLicense {
	name: string;
	versions: string[];
}

/** Whether a simple SPDX expression (`A`, `A OR B`, `A AND B`, optionally parenthesized) is allowed. */
export function isAllowed(expression: string): boolean {
	const expr = expression.trim().replace(/^\((.*)\)$/, '$1');
	const hasOr = expr.includes(' OR ');
	const hasAnd = expr.includes(' AND ');
	if (hasOr && hasAnd) {
		// Mixed expressions need a human decision; add an exact entry to ALLOWED if acceptable.
		return ALLOWED.has(expr);
	}
	if (hasOr) {
		return expr.split(' OR ').some((part) => ALLOWED.has(part.trim()));
	}
	if (hasAnd) {
		return expr.split(' AND ').every((part) => ALLOWED.has(part.trim()));
	}
	return ALLOWED.has(expr);
}

function main(): void {
	const output = execFileSync('pnpm', ['licenses', 'list', '--prod', '--json'], {
		encoding: 'utf8'
	});
	const byLicense = JSON.parse(output) as Record<string, PackageLicense[]>;
	let rejected = 0;
	for (const [license, packages] of Object.entries(byLicense)) {
		const status = isAllowed(license) ? 'ok' : 'REJECTED';
		for (const pkg of packages) {
			console.log(
				`${status.padEnd(8)} ${license.padEnd(24)} ${pkg.name}@${pkg.versions.join(',')}`
			);
			if (status === 'REJECTED') rejected += 1;
		}
	}
	if (rejected > 0) {
		console.error(
			`${String(rejected)} package(s) with a license not allowed for an MIT OR Apache-2.0 app`
		);
		process.exit(1);
	}
}

if (import.meta.main) main();
