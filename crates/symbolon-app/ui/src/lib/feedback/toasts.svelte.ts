/** How a toast looks and how urgently it is announced. */
export type ToastTone = 'info' | 'success' | 'warning' | 'error';

/** One shown toast. */
export interface ToastItem {
	readonly id: number;
	readonly message: string;
	readonly tone: ToastTone;
}

/** A queue of toasts; errors stay until dismissed, others leave after their timeout. */
export interface Toasts {
	readonly items: readonly ToastItem[];
	show(message: string, tone?: ToastTone, timeoutMs?: number): number;
	dismiss(id: number): void;
}

/** Creates an empty toast queue. */
export function createToasts(): Toasts {
	let items = $state<ToastItem[]>([]);
	let next = 1;
	const dismiss = (id: number) => {
		items = items.filter((t) => t.id !== id);
	};
	return {
		get items() {
			return items;
		},
		show(message, tone = 'info', timeoutMs = 5000) {
			const id = next++;
			items = [...items, { id, message, tone }];
			// A failure must not disappear before it is read.
			if (tone !== 'error') {
				setTimeout(() => {
					dismiss(id);
				}, timeoutMs);
			}
			return id;
		},
		dismiss
	};
}

/** The app's toast queue. */
export const toasts = createToasts();
