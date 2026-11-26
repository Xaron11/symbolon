// Components and routes use design tokens only (ADR-0022): no raw colors, and no raw lengths or
// durations for spacing, borders, outlines, radii, type or motion. Token arithmetic such as
// `calc(var(--sy-tree-indent) * 3)` is allowed. src/lib/design/ defines the base styles and is
// exempt. Tested in scripts/stylelint-rules.test.ts.
const RAW_UNITS = ['px', 'rem', 'em', 'pt', 'ms', 's'];

/** @type {import('stylelint').Config} */
export default {
	plugins: ['stylelint-declaration-strict-value'],
	overrides: [{ files: ['**/*.svelte'], customSyntax: 'postcss-html' }],
	rules: {
		'color-no-hex': true,
		'color-named': 'never',
		'function-disallowed-list': [
			'rgb',
			'rgba',
			'hsl',
			'hsla',
			'hwb',
			'lab',
			'lch',
			'oklab',
			'oklch'
		],
		'declaration-property-unit-disallowed-list': {
			'/^(margin|padding|inset|top|right|bottom|left)/': RAW_UNITS,
			'/^(row-|column-)?gap$/': RAW_UNITS,
			'/^(border|outline)/': RAW_UNITS,
			'/^font/': RAW_UNITS,
			'/^(line-height|letter-spacing)$/': RAW_UNITS,
			'/^(transition|animation)/': RAW_UNITS
		},
		'scale-unlimited/declaration-strict-value': [
			['/color$/', 'background', 'fill', 'stroke', 'font-family', 'line-height', 'box-shadow'],
			{
				ignoreValues: ['transparent', 'currentColor', 'inherit', 'none', 'auto', '0', '100%'],
				disableFix: true
			}
		]
	},
	ignoreFiles: ['src/lib/design/**', 'build/**', '.svelte-kit/**']
};
