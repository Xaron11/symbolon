import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDispatcher, isTextEntry } from './dispatcher';
import { KEYMAP, type Action, type KeyContext } from './keymap';

let contexts: KeyContext[] = ['global'];
let fired: Action[] = [];
let handle: (event: KeyboardEvent) => boolean;

const press = (key: string, init: KeyboardEventInit = {}, target: EventTarget = document.body) => {
	const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
	Object.defineProperty(event, 'target', { value: target });
	return { handled: handle(event), prevented: event.defaultPrevented };
};

beforeEach(() => {
	vi.useFakeTimers();
	contexts = ['global'];
	fired = [];
	handle = createDispatcher(
		KEYMAP,
		() => contexts,
		(action) => fired.push(action)
	);
});
afterEach(() => vi.useRealTimers());

describe('createDispatcher', () => {
	it('maps chords', () => {
		expect(press('k', { ctrlKey: true })).toEqual({ handled: true, prevented: true });
		press('1', { ctrlKey: true });
		press('k', { metaKey: true });
		expect(fired).toEqual(['palette.open', 'nav.locations', 'palette.open']);
	});

	it('maps symbols without counting the Shift that types them', () => {
		press('?', { shiftKey: true });
		expect(fired).toEqual(['help.open']);
	});

	it('maps vi keys only in a list context', () => {
		expect(press('j').handled).toBe(false);
		contexts = ['global', 'list'];
		press('j');
		press('ArrowDown');
		press('G', { shiftKey: true });
		press(' ');
		expect(fired).toEqual(['list.down', 'list.down', 'list.bottom', 'list.toggleSelect']);
	});

	it('handles sequences', () => {
		contexts = ['global', 'list'];
		expect(press('g').handled).toBe(true);
		press('l');
		press('g');
		press('g');
		expect(fired).toEqual(['nav.locations', 'list.top']);
	});

	it('lets a sequence time out', () => {
		contexts = ['global', 'list'];
		press('g');
		vi.advanceTimersByTime(1001);
		press('l');
		expect(fired).toEqual(['list.expand']);
	});

	it('drops an unfinished sequence and handles the next key alone', () => {
		contexts = ['global', 'list'];
		press('g');
		press('j');
		expect(fired).toEqual(['list.down']);
	});

	it('ignores letters while text is entered, but not modifier chords', () => {
		contexts = ['global', 'list'];
		const input = document.createElement('input');
		expect(press('j', {}, input).handled).toBe(false);
		expect(press('/', {}, input).handled).toBe(false);
		press('k', { ctrlKey: true }, input);
		expect(fired).toEqual(['palette.open']);
	});

	it('ignores IME composition', () => {
		contexts = ['global', 'list'];
		expect(press('j', { isComposing: true }).handled).toBe(false);
		expect(press('j', { keyCode: 229 }).handled).toBe(false);
		expect(fired).toEqual([]);
	});

	it('leaves events already handled to their handler', () => {
		contexts = ['global', 'list'];
		const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
		event.preventDefault();
		expect(handle(event)).toBe(false);
		expect(fired).toEqual([]);
	});

	it.each(['dialog', 'menu', 'listbox'])('leaves keys inside a %s to it', (role) => {
		contexts = ['global', 'list'];
		const area = document.createElement('div');
		area.setAttribute('role', role);
		const button = document.createElement('button');
		area.append(button);
		for (const [key, init] of [
			['g', {}],
			['l', {}],
			['1', { ctrlKey: true }],
			['k', { ctrlKey: true }],
			['Escape', {}]
		] as const) {
			expect(press(key, init, button).handled, key).toBe(false);
		}
		expect(fired).toEqual([]);
	});

	it('uses the physical key on non-Latin layouts', () => {
		contexts = ['global', 'list'];
		press('л', { ctrlKey: true, code: 'KeyK' });
		press('о', { code: 'KeyJ' });
		press('&', { ctrlKey: true, code: 'Digit1' });
		expect(fired).toEqual(['palette.open', 'list.down', 'nav.locations']);
	});

	it('keeps the typed letter on Latin layouts such as Dvorak', () => {
		contexts = ['global', 'list'];
		press('j', { code: 'KeyC' });
		expect(fired).toEqual(['list.down']);
	});

	it('ignores events without a key, such as autofill', () => {
		const event = new KeyboardEvent('keydown');
		Object.defineProperty(event, 'key', { value: undefined });
		expect(() => handle(event)).not.toThrow();
		expect(handle(event)).toBe(false);
	});

	it('ignores lone modifier keys', () => {
		expect(press('Shift', { shiftKey: true }).handled).toBe(false);
	});
});

describe('isTextEntry', () => {
	it.each([
		['text input', () => document.createElement('input'), true],
		['textarea', () => document.createElement('textarea'), true],
		['checkbox', () => Object.assign(document.createElement('input'), { type: 'checkbox' }), false],
		['button', () => document.createElement('button'), false],
		[
			'editable text',
			() => {
				const div = document.createElement('div');
				div.setAttribute('contenteditable', 'true');
				return div;
			},
			true
		],
		[
			'inside a text-entry area',
			() => {
				const area = document.createElement('div');
				area.dataset['syTextEntry'] = '';
				const child = document.createElement('span');
				area.append(child);
				return child;
			},
			true
		]
	] as const)('%s → %s', (_, make, expected) => {
		expect(isTextEntry(make())).toBe(expected);
	});
});
