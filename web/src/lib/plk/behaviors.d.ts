export type ThemeChoice = 'light' | 'dark' | 'system';
export type Tone = 'success' | 'warning' | 'info' | 'destructive';

export interface ToastOptions {
    title: string;
    description?: string;
    variant?: Tone;
    /** ms; 0 keeps the toast until dismissed. Default 4000 (destructive 8000). */
    duration?: number;
    action?: { label: string; onClick?: () => void };
}

export interface ConfirmOptions {
    title?: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: 'danger' | 'warning';
}

export declare const strings: Record<
    | 'confirmTitle' | 'confirm' | 'cancel' | 'close' | 'notifications' | 'showPassword' | 'hidePassword'
    | 'updated' | 'online' | 'offline' | 'degraded' | 'fileTooLarge' | 'fileWrongType'
    | 'removeItem' | 'otpDigit' | 'secure' | 'insecure',
    string
>;

export type PrefName = 'mode' | 'density' | 'fontScale' | 'texture' | 'motion' | 'rail';

export declare function init(scope?: ParentNode): void;
/** Current value of a shared preference (mode, density, fontScale, texture, motion, rail). */
export declare function getPref(name: PrefName): string;
/** Apply, persist and (with <html data-plk-prefs>) share a preference; emits `plk:pref`. */
export declare function setPref(name: PrefName, value: string | null, options?: { persist?: boolean }): void;
export interface AppTile { id?: string; name: string; url?: string; href?: string; profile?: string; mark?: string; image?: string; description?: string; meta?: string }
/** Build the app-launcher grid inside `host` (also runs for [data-plk-app-switcher] on init). */
export declare function renderAppSwitcher(host: Element, apps: AppTile[]): void;
/** `url` with the current preferences appended as #plk=… (#vibe=… when data-plk-prefs="vibe"). */
export declare function handoff(url: string): string;
export declare function getTheme(): ThemeChoice;
export declare function resolveTheme(choice?: ThemeChoice): 'light' | 'dark';
export declare function setTheme(choice: ThemeChoice, options?: { animate?: boolean }): void;
export declare function toggleTheme(): void;
export declare function setProfile(name: string | null): void;
export declare function setPalette(name: string | null): void;
export declare function initReveal(scope?: ParentNode): void;
export declare function openDialog(target: string | HTMLElement, opener?: HTMLElement | null): HTMLElement | null;
export declare function closeDialog(target: string | HTMLElement, value?: string): void;
export declare function openMenuPanel(root: Element, options?: { focus?: 'first' | 'last' | null }): void;
export declare function closeMenuPanel(root?: Element | null, options?: { restoreFocus?: boolean }): void;
export declare function toast(options: string | ToastOptions): string;
export declare function dismissToast(target: string | HTMLElement): void;
export declare function confirm(options: string | ConfirmOptions): Promise<boolean>;
export declare function selectTab(tab: Element, options?: { focus?: boolean }): void;
export declare function toggleSidebar(shell?: Element | null, force?: boolean): void;
export declare function renderRefreshed(scope?: ParentNode): void;

export interface IdleOptions {
    /** seconds of inactivity before sign-out. Default 900. */
    timeout?: number;
    /** seconds before the timeout the dialog opens. Default 60. */
    warn?: number;
    /** warning dialog (element or id) holding [data-plk-idle-count]. */
    dialog?: string | HTMLElement | null;
    /** replaces the default sign-out ([data-plk-idle-logout] link or form). */
    onTimeout?: (() => void) | null;
}

export declare function openContextMenu(menu: string | HTMLElement, x: number, y: number, options?: { region?: Element | null }): void;
export declare function closeContextMenu(options?: { restoreFocus?: boolean }): void;
/** Fill an OTP group; returns the joined value. */
export declare function setOtp(root: Element, text: string): string;
/** Percent sizes, normalised to 100. */
export declare function setPanelSizes(root: Element, sizes: number[], options?: { persist?: boolean }): void;
/** Returns stopIdle. */
export declare function startIdle(options?: IdleOptions): () => void;
export declare function stopIdle(): void;
export declare function resetIdle(): void;
export declare function getLocale(): string;
export declare function setLocale(code: string): void;

declare const Plk: {
    init: typeof init;
    strings: typeof strings;
    getTheme: typeof getTheme;
    resolveTheme: typeof resolveTheme;
    setTheme: typeof setTheme;
    toggleTheme: typeof toggleTheme;
    setProfile: typeof setProfile;
    setPalette: typeof setPalette;
    initReveal: typeof initReveal;
    openDialog: typeof openDialog;
    closeDialog: typeof closeDialog;
    openMenuPanel: typeof openMenuPanel;
    closeMenuPanel: typeof closeMenuPanel;
    toast: typeof toast;
    dismissToast: typeof dismissToast;
    confirm: typeof confirm;
    selectTab: typeof selectTab;
    toggleSidebar: typeof toggleSidebar;
    renderRefreshed: typeof renderRefreshed;
    openContextMenu: typeof openContextMenu;
    closeContextMenu: typeof closeContextMenu;
    setOtp: typeof setOtp;
    setPanelSizes: typeof setPanelSizes;
    startIdle: typeof startIdle;
    stopIdle: typeof stopIdle;
    resetIdle: typeof resetIdle;
    getLocale: typeof getLocale;
    setLocale: typeof setLocale;
};
export default Plk;

declare global {
    interface Window {
        Plk?: typeof Plk;
        PLK_MANUAL?: boolean;
    }
}
