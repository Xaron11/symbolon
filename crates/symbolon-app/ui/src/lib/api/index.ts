// The frontend API layer (ADR-0021 §2): screens import from here, never from Tauri directly.
export type { Backend } from './backend';
export { ApiError, toApiError } from './errors';
export { backend, provideBackend } from './context';
export { createTauriBackend } from './tauri';
export {
	createMemoryBackend,
	type BackendCall,
	type MemoryBackend,
	type MemoryComparison,
	type MemorySeed
} from './memory';
export {
	catalogTree,
	comparisonRows,
	comparisonTree,
	createChangeHub,
	duplicateGroups,
	locationScans,
	recentScans,
	rowOrder,
	scanWarnings,
	type ChangeHub,
	type DuplicateNode
} from './sources';
export type * from './types';
