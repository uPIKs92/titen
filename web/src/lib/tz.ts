// ── Timezone utilities ──
// Reads timezone from backend /health endpoint.
// Falls back to browser timezone if unavailable.

import { getHealth } from './api';

/** Display timezone: always the browser's, whatever the server's TZ is. */
export const DEFAULT_TIMEZONE: string = (() => {
	try {
		return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
	} catch {
		return 'UTC';
	}
})();

function normalizeTz(_serverTz: string | null | undefined): string {
	return DEFAULT_TIMEZONE;
}

let cachedTz: string | null = null;
let fetchPromise: Promise<string> | null = null;

/**
 * Fetch timezone from backend /health endpoint.
 * Caches the result for subsequent calls.
 * Returns IANA timezone string (the browser's timezone).
 */
export async function fetchTimezone(): Promise<string> {
	if (cachedTz) return cachedTz;
	if (fetchPromise) return fetchPromise;

	fetchPromise = getHealth()
		.then((h) => {
			cachedTz = normalizeTz(h.timezone);
			return cachedTz;
		})
		.catch(() => {
			// Do NOT cache the browser fallback — retry on next call
			// so the server TZ wins once /health is reachable.
			return DEFAULT_TIMEZONE;
		})
		.finally(() => {
			fetchPromise = null;
		});

	return fetchPromise;
}

/**
 * Get cached timezone (sync). Returns null if not yet fetched.
 */
export function getTimezone(): string | null {
	return cachedTz;
}

/**
 * Set timezone manually (useful for tests or SSR).
 */
export function setTimezone(tz: string): void {
	cachedTz = tz;
}

/**
 * Format an ISO datetime string for display using the configured timezone.
 * Falls back to browser timezone if TZ not yet fetched.
 */
export function formatDateTime(iso: string, timezone?: string | null): string {
	const d = new Date(iso);
	const tz = timezone || cachedTz || DEFAULT_TIMEZONE;
	return d.toLocaleString('en-US', {
		month: 'short',
		day: 'numeric',
		year: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		timeZone: tz,
	});
}

/**
 * Format an ISO datetime string as a date only (no time).
 */
export function formatDate(iso: string, timezone?: string | null): string {
	const d = new Date(iso);
	const tz = timezone || cachedTz || DEFAULT_TIMEZONE;
	return d.toLocaleDateString('en-US', {
		month: 'short',
		day: 'numeric',
		year: 'numeric',
		timeZone: tz,
	});
}

/**
 * Format an ISO datetime string as a short date (month + day only, no year).
 */
export function formatDateShort(iso: string, timezone?: string | null): string {
	const d = new Date(iso);
	const tz = timezone || cachedTz || DEFAULT_TIMEZONE;
	return d.toLocaleDateString('en-US', {
		month: 'short',
		day: 'numeric',
		timeZone: tz,
	});
}

/**
 * Format an ISO datetime as date + time without year (e.g. "Aug 9, 2:30 PM").
 */
export function formatDateTimeShort(iso: string, timezone?: string | null): string {
	const d = new Date(iso);
	const tz = timezone || cachedTz || DEFAULT_TIMEZONE;
	return d.toLocaleString('en-US', {
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		timeZone: tz,
	});
}

/**
 * Convert an ISO datetime to a local datetime-local input value.
 * Uses the configured timezone for conversion.
 */
export function toDatetimeInput(iso: string, timezone?: string | null): string {
	const d = new Date(iso);
	const tz = timezone || cachedTz || DEFAULT_TIMEZONE;
	// Format in target TZ, then extract parts
	const parts = new Intl.DateTimeFormat('en-US', {
		year: 'numeric',
		month: '2-digit',
		day: '2-digit',
		hour: '2-digit',
		minute: '2-digit',
		hour12: false,
		timeZone: tz,
	}).formatToParts(d);

	const get = (type: string) => parts.find((p) => p.type === type)?.value ?? '00';
	return `${get('year')}-${get('month')}-${get('day')}T${get('hour')}:${get('minute')}`;
}
