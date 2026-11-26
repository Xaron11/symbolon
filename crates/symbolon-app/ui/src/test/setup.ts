// jsdom lacks APIs the app reads at import time or that Bits UI uses.
if (typeof window.matchMedia !== 'function') {
	window.matchMedia = (query: string): MediaQueryList =>
		({
			matches: false,
			media: query,
			onchange: null,
			addEventListener: () => undefined,
			removeEventListener: () => undefined,
			dispatchEvent: () => false
		}) as unknown as MediaQueryList;
}

if (typeof globalThis.ResizeObserver !== 'function') {
	globalThis.ResizeObserver = class {
		observe() {
			// Layout never changes in jsdom.
		}
		unobserve() {
			// Nothing observed.
		}
		disconnect() {
			// Nothing observed.
		}
	};
}
