import { vi } from 'vitest';

// Test files share a worker (isolate: false). Without this, a module an earlier file imported
// stays loaded, and a later file's vi.mock of one of its imports never reaches it.
vi.resetModules();

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

// jsdom has no layout: elements measure 0×0 and have no client rects, so Bits UI treats every
// anchor as hidden and never shows floating content. Give elements one nominal box instead.
const box = (): DOMRect => ({
	x: 0,
	y: 0,
	top: 0,
	left: 0,
	right: 100,
	bottom: 20,
	width: 100,
	height: 20,
	toJSON: () => ({})
});
Element.prototype.getBoundingClientRect = box;
Element.prototype.getClientRects = function getClientRects(): DOMRectList {
	const list = [box()];
	return Object.assign(list, { item: (i: number) => list[i] ?? null });
};

// Bits UI scrolls the active command into view; jsdom has nothing to scroll.
Element.prototype.scrollIntoView = function scrollIntoView() {
	// No layout in jsdom.
};
