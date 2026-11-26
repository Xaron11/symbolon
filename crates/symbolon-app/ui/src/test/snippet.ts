import { createRawSnippet, type Snippet } from 'svelte';

/** A snippet rendering `content` as text inside a span, for passing children in tests. */
export function text(content: string): Snippet {
	return createRawSnippet(() => ({
		render: () => `<span>${content.replaceAll('<', '&lt;')}</span>`
	}));
}
