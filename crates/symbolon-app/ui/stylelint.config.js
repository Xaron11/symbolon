// Components and routes use design tokens only (ADR-0022): no raw colors, lengths or font
// values. Design files that define the tokens' use (src/lib/design/) are exempt.
/** @type {import('stylelint').Config} */
export default {
	plugins: ['stylelint-declaration-strict-value'],
	overrides: [{ files: ['**/*.svelte'], customSyntax: 'postcss-html' }],
	rules: {
		'scale-unlimited/declaration-strict-value': [
			[
				'/color$/',
				'background',
				'fill',
				'stroke',
				'/^(margin|padding|gap|inset|top|right|bottom|left)/',
				'/^border-(.*-)?(radius|width)$/',
				'font-size',
				'font-family',
				'line-height',
				'box-shadow'
			],
			{
				ignoreValues: ['transparent', 'currentColor', 'inherit', 'none', 'auto', '0', '100%'],
				ignoreFunctions: false,
				disableFix: true
			}
		]
	},
	ignoreFiles: ['src/lib/design/**', 'build/**', '.svelte-kit/**']
};
