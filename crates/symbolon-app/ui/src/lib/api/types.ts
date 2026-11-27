// The shapes the backend sends and accepts. tauri-specta generates a `_Serialize` variant (what
// Rust sends: optional values always present, maybe null) and a `_Deserialize` variant (what
// Rust accepts: optional values may be left out) for types with optional fields; screens use
// these names instead.
import type * as B from './bindings';

/** A saved location as the Locations screen shows it. */
export type LocationView = B.LocationView_Serialize;
/** Whether a location can be reached now. */
export type LocationStatus = B.LocationStatus;
/** Where a location's files live. */
export type LocationKind = B.LocationKind;
/** When a location's files are hashed. */
export type HashPolicy = B.HashPolicyView;
/** The Add location form. */
export type NewLocationRequest = B.NewLocationRequest_Deserialize;
/** What a location's catalog holds. */
export type StatsView = B.StatsView;
/** One recorded scan. */
export type ScanBrief = B.ScanBrief_Serialize;
/** A recorded scan of any location. */
export type RecentScanView = B.RecentScanView_Serialize;
/** A problem a scan recorded. */
export type WarningView = B.WarningView_Serialize;
/** A phone adb can see. */
export type DeviceView = B.DeviceView_Serialize;
/** One message about a running scan. */
export type ScanUpdate = B.ScanUpdate;
/** How a scan ended, or that it runs. */
export type ScanStatus = B.ScanStatusView;
/** A location's catalog changed under a browser. */
export type BrowseChange = B.BrowseChange;
/** An entry of a location's catalog. */
export type EntryView = B.EntryView_Serialize;
/** The comparison picker's choices. */
export type CompareRequest = B.CompareRequest_Deserialize;
/** One side of a comparison. */
export type SideRequest = B.SideRequest_Serialize;
/** An opened comparison. */
export type CompareOpened = B.CompareOpened_Serialize;
/** Which comparison rows to read; unset fields match everything. */
export type ApiFilter = B.ApiFilter_Deserialize;
/** A row order. */
export type RowOrder = B.RowOrderView;
/** A comparison category, named as core names it. */
export type CategoryName = B.CategoryName;
/** Totals by category. */
export type SummaryView = B.SummaryView;
/** Rows, entries and bytes of one category. */
export type CategoryTotals = B.CategoryTotals;
/** A classified row. */
export type CompareRowView = B.CompareRowView_Serialize;
/** One side's entry in a row. */
export type CellView = B.CellView_Serialize;
/** A child in the comparison tree. */
export type TreeNodeView = B.TreeNodeView_Serialize;
/** Rows and bytes of one extension. */
export type ExtensionView = B.ExtensionView_Serialize;
/** Rows and bytes in one size bucket. */
export type SizeBucketView = B.SizeBucketView_Serialize;
/** Files with the same content within one side. */
export type DuplicateSetView = B.DuplicateSetView_Serialize;
/** Duplicate totals of one side. */
export type DuplicateTotalsView = B.DuplicateTotalsView;
/** Background verification of a comparison. */
export type VerifyUpdate = B.VerifyUpdate;
/** What went wrong in a command. */
export type ErrorKind = B.ErrorKind;
