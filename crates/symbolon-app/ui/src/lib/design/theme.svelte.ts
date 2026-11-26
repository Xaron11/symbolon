import type { ThemeName } from './generated/tokens';

/** What the user chose: follow the OS, or a fixed theme. */
export type ThemePreference = 'system' | 'dark' | 'light';

/** The reactive theme state; `resolved` is the theme actually shown. */
export interface Theme {
	readonly preference: ThemePreference;
	readonly resolved: ThemeName;
	set(preference: ThemePreference): void;
	destroy(): void;
}

const KEY = 'symbolon.theme';
const PREFERENCES: readonly ThemePreference[] = ['system', 'dark', 'light'];

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>;

function read(storage: Storage | undefined): ThemePreference {
	try {
		const value = storage?.getItem(KEY);
		return PREFERENCES.find((p) => p === value) ?? 'system';
	} catch {
		return 'system';
	}
}

/**
 * Creates the theme state from a stored preference and the OS color scheme (`media`, a
 * `prefers-color-scheme: dark` query), and applies it as `data-theme` on the root element
 * (absent while following the OS). Storage failures leave the preference unsaved.
 */
export function createTheme(storage: Storage | undefined, media: MediaQueryList): Theme {
	let preference = $state(read(storage));
	let systemDark = $state(media.matches);
	const resolved: ThemeName = $derived(
		preference === 'system' ? (systemDark ? 'dark' : 'light') : preference
	);

	const apply = () => {
		const root = document.documentElement;
		if (preference === 'system') delete root.dataset['theme'];
		else root.dataset['theme'] = preference;
	};
	const onChange = (event: { matches: boolean }) => {
		systemDark = event.matches;
	};
	media.addEventListener('change', onChange);
	apply();

	return {
		get preference() {
			return preference;
		},
		get resolved() {
			return resolved;
		},
		set(next) {
			preference = next;
			apply();
			try {
				storage?.setItem(KEY, next);
			} catch {
				// The choice still applies for this session.
			}
		},
		destroy() {
			media.removeEventListener('change', onChange);
		}
	};
}

function browserStorage(): Storage | undefined {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
}

/** The app's theme, following `localStorage` and the OS. Undefined outside a browser. */
export const theme: Theme | undefined =
	typeof window === 'undefined' || typeof window.matchMedia !== 'function'
		? undefined
		: createTheme(browserStorage(), window.matchMedia('(prefers-color-scheme: dark)'));
