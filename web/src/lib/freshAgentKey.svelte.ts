// The plaintext of a just-created agent key. The server never shows it again, so
// keep it in memory only (not localStorage/sessionStorage): it survives tab
// switches within the Settings page but is gone after a reload.
let current = $state<{ name: string; value: string } | null>(null);

export const freshAgentKey = {
	get current() {
		return current;
	},
	set(key: { name: string; value: string }) {
		current = key;
	},
	clear() {
		current = null;
	},
};
