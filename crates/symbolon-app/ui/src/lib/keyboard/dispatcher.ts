import type { Action, Binding, KeyContext } from './keymap';

const MODIFIER_KEYS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'AltGraph']);
const NON_TEXT_INPUTS = new Set([
	'checkbox',
	'radio',
	'button',
	'submit',
	'reset',
	'range',
	'color',
	'file'
]);

/** Whether `target` takes typed text, where single-key shortcuts must not act. */
export function isTextEntry(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target.closest('[data-sy-text-entry]')) return true;
	if (target instanceof HTMLInputElement) return !NON_TEXT_INPUTS.has(target.type);
	return (
		target instanceof HTMLTextAreaElement ||
		target instanceof HTMLSelectElement ||
		target.closest('[contenteditable]:not([contenteditable="false"])') !== null
	);
}

/** Widgets that handle their own keys (typeahead, Escape, arrows); shortcuts stay out of them. */
const OWN_KEYS = '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]';

/**
 * The key `event` names: the typed character on Latin layouts (so Dvorak users keep their
 * letters), the physical key where the character is not ASCII (Cyrillic, Greek, …), and the
 * digit of a digit key under Ctrl or Alt (AZERTY types "&" there).
 */
function baseKey(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey'>) {
	const physical = /^(?:Key([A-Z])|Digit(\d))$/.exec(event.code);
	const fromCode = physical ? (physical[1] ?? physical[2] ?? '').toLowerCase() : undefined;
	if (event.key.length === 1 && !/^[\x20-\x7e]$/.test(event.key) && fromCode) return fromCode;
	if ((event.ctrlKey || event.metaKey || event.altKey) && physical?.[2]) return physical[2];
	return event.key === ' ' ? 'space' : event.key.toLowerCase();
}

/** The key of `event` in keymap notation, or undefined for a lone modifier or no key at all. */
export function keyOf(
	event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>
): string | undefined {
	// Autofill and some IMEs send keydown events without a key.
	if (typeof event.key !== 'string' || event.key === '' || MODIFIER_KEYS.has(event.key)) {
		return undefined;
	}
	const key = baseKey(event);
	const parts: string[] = [];
	if (event.ctrlKey || event.metaKey) parts.push('ctrl');
	if (event.altKey) parts.push('alt');
	// Shift belongs to the chord for letters and named keys; for symbols such as "?" it is just
	// how the symbol is typed.
	if (event.shiftKey && (/^[a-z]$/.test(key) || key.length > 1)) parts.push('shift');
	parts.push(key);
	return parts.join('+');
}

/**
 * Creates a keydown handler that runs `onAction` for the bindings of the active contexts.
 * Sequences (`g l`) wait up to `sequenceTimeoutMs` for their next key. While text is entered,
 * or an IME is composing, only chords with Ctrl or Alt act. Returns whether the event was
 * handled; handled events have their default prevented.
 */
export function createDispatcher(
	bindings: readonly Binding[],
	contexts: () => readonly KeyContext[],
	onAction: (action: Action) => void,
	options: { sequenceTimeoutMs?: number } = {}
): (event: KeyboardEvent) => boolean {
	const timeout = options.sequenceTimeoutMs ?? 1000;
	let pending: string[] = [];
	let timer: ReturnType<typeof setTimeout> | undefined;

	const reset = () => {
		pending = [];
		clearTimeout(timer);
	};

	const match = (sequence: string) => {
		const active = contexts();
		let prefix = false;
		for (const binding of bindings) {
			if (!active.includes(binding.context)) continue;
			for (const key of binding.keys) {
				if (key === sequence) return { action: binding.action, prefix: false };
				if (key.startsWith(`${sequence} `)) prefix = true;
			}
		}
		return { action: undefined, prefix };
	};

	return (event) => {
		// keyCode 229 is how some engines still flag IME input without isComposing.
		// eslint-disable-next-line @typescript-eslint/no-deprecated -- the only signal there
		if (event.isComposing || event.keyCode === 229) return false;
		if (event.defaultPrevented) return false;
		if (event.target instanceof Element && event.target.closest(OWN_KEYS)) {
			reset();
			return false;
		}
		const key = keyOf(event);
		if (key === undefined) return false;
		if (isTextEntry(event.target) && !/^(ctrl|alt)\+/.test(key)) {
			reset();
			return false;
		}

		for (const sequence of pending.length > 0 ? [[...pending, key], [key]] : [[key]]) {
			const found = match(sequence.join(' '));
			if (found.action !== undefined) {
				reset();
				event.preventDefault();
				onAction(found.action);
				return true;
			}
			if (found.prefix) {
				reset();
				pending = sequence;
				timer = setTimeout(reset, timeout);
				event.preventDefault();
				return true;
			}
		}
		reset();
		return false;
	};
}
