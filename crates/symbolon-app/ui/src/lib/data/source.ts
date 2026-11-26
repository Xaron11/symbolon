// The contract between data components and whatever serves the rows: the app–core bridge over
// Tauri, an HTTP backend later, or memory in tests and the gallery. Offset-based, so a list can
// jump anywhere without reading the pages before it (ADR-0022).

/** A sort order requested by a table. */
export interface SortSpec {
	column: string;
	direction: 'asc' | 'desc';
}

/** Rows of a list, read by offset. */
export interface DataSource<Row> {
	/** Total number of rows. */
	count(): Promise<number>;
	/** Up to `limit` rows from `offset`, in `sort` order (the source's own order if undefined). */
	rows(offset: number, limit: number, sort?: SortSpec): Promise<readonly Row[]>;
}

/** Nodes of a tree, read level by level. */
export interface TreeSource<Node> {
	rootCount(): Promise<number>;
	roots(offset: number, limit: number): Promise<readonly Node[]>;
	childCount(node: Node): Promise<number>;
	children(node: Node, offset: number, limit: number): Promise<readonly Node[]>;
	/** A stable identity for `node`. */
	key(node: Node): string;
	/** Whether `node` can have children (a folder), even before they are read. */
	hasChildren(node: Node): boolean;
}
