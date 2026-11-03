<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { toast } from '$lib/toast.svelte';

	let {
		redirectUri,
		onclose,
		onadd,
	}: {
		redirectUri: string;
		onclose: () => void;
		/** Jump straight to the Add Account dialog. */
		onadd?: () => void;
	} = $props();

	const appDomain = $derived.by(() => {
		try {
			return new URL(redirectUri).host;
		} catch {
			return '';
		}
	});

	let copied = $state<string | null>(null);
	async function copy(text: string, key: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = key;
			setTimeout(() => (copied = null), 1500);
		} catch {
			toast('Copy failed — select the text and copy manually', 'error');
		}
	}

	const scopes = [
		'threads_basic',
		'threads_content_publish',
		'threads_manage_replies',
		'threads_manage_mentions',
		'threads_keyword_search',
		'threads_profile_discovery',
		'threads_share_to_instagram',
		'threads_location_tagging',
	];
</script>

<Dialog.Root open onOpenChange={(o) => { if (!o) onclose(); }}>
	<Dialog.Content class="confirm-dialog guide" style="max-width:40rem;max-height:88vh;overflow-y:auto;" aria-describedby={undefined}>
		<Dialog.Title class="text-base font-semibold mb-xs">Connect a Threads account — Meta setup guide</Dialog.Title>
		<p class="text-hint mb-sm">
			Every account is connected through a Threads app you own on Meta. Do this once per app; accounts that share an app reuse it.
			Meta renames menus from time to time, so match by label if a page looks different.
		</p>

		<ol class="guide-steps">
			<li>
				<h3>Open your app</h3>
				<p>
					Go to <a href="https://developers.facebook.com/apps/" target="_blank" rel="noopener noreferrer">developers.facebook.com/apps</a>
					and open your app. If it has no Threads use case, choose <strong>Add use case → Access the Threads API</strong>.
				</p>
			</li>

			<li>
				<h3>Copy the Threads app ID and secret</h3>
				<p class="guide-path">Use cases → <strong>Access the Threads API</strong> → <strong>Customize</strong> → <strong>Settings</strong></p>
				<p>
					Copy <strong>Threads app ID</strong> and <strong>Threads app secret</strong> into Titen (Add Account → step 1, or <em>Edit</em> on the account).
				</p>
				<p class="guide-warn">
					Not <em>App settings → Basic</em>. That page shows the main Meta app ID, which Threads rejects
					(<code>Invalid client_id</code> / “No app ID was sent”).
				</p>
			</li>

			<li>
				<h3>Allow Titen's redirect URI</h3>
				<p class="guide-path">Same Settings page → <strong>Redirect Callback URLs</strong> → add this exact value → <strong>Save</strong></p>
				<div class="guide-copy">
					<code>{redirectUri}</code>
					<Button type="button" variant="outline" size="sm" onclick={() => copy(redirectUri, 'uri')}>{copied === 'uri' ? 'Copied' : 'Copy'}</Button>
				</div>
				<p>Exact match only: <code>https</code>, no trailing slash, no spaces. A leftover <code>localhost</code> entry does not count.</p>
			</li>

			<li>
				<h3>If Meta says “URL Blocked” (error 1349168)</h3>
				<p>Also set these two, then retry:</p>
				<ul>
					<li>
						<strong>Facebook Login → Settings</strong>: <em>Client OAuth Login</em> = Yes, <em>Web OAuth Login</em> = Yes, and add the same URI to
						<em>Valid OAuth Redirect URIs</em>. Its <em>Redirect URI Validator</em> box tells you whether Meta accepts it.
					</li>
					<li>
						<strong>App settings → Basic → App Domains</strong>: add
						{#if appDomain}
							<code>{appDomain}</code>
							<button type="button" class="guide-link" onclick={() => copy(appDomain, 'dom')}>{copied === 'dom' ? 'copied' : 'copy'}</button>
						{:else}your domain{/if}.
					</li>
				</ul>
			</li>

			<li>
				<h3>Add the Threads account as a tester</h3>
				<p class="guide-path"><strong>App roles → Roles → Add People</strong> → <strong>Threads Tester</strong> → enter the Threads username</p>
				<p>
					While the app is in development mode, only testers can authorize. The account owner accepts the invite on
					<a href="https://www.threads.com/" target="_blank" rel="noopener noreferrer">threads.com</a> →
					<strong>Settings → Account → Website permissions → Invites</strong>.
				</p>
			</li>

			<li>
				<h3>Permissions</h3>
				<p>Titen requests these scopes. In development mode no App Review is needed for testers.</p>
				<p class="guide-scopes">{#each scopes as s}<code>{s}</code>{/each}</p>
			</li>

			<li>
				<h3>Connect</h3>
				<p>Back in Titen: <strong>Add Account → Connect with Threads</strong> (or <strong>Reconnect</strong> on an expired account), then click Authorize on Threads.</p>
			</li>
		</ol>

		<h3 class="guide-sub">Troubleshooting</h3>
		<table class="guide-table">
			<thead><tr><th>Meta says</th><th>Usually means</th></tr></thead>
			<tbody>
				<tr><td><code>1349168</code> URL Blocked / redirect URI not whitelisted</td><td>Step 3 and 4: the URI isn't saved exactly as shown above.</td></tr>
				<tr><td><code>4476002</code> No app ID was sent</td><td>Main Meta app ID used instead of the Threads app ID (step 2), or a truncated link.</td></tr>
				<tr><td><code>Invalid client_id</code></td><td>Wrong app ID, or not a Threads app ID.</td></tr>
				<tr><td>Not allowed to authorize / app not active</td><td>Account isn't a Threads tester yet (step 5), or the invite wasn't accepted.</td></tr>
			</tbody>
		</table>

		<div class="confirm-actions">
			{#if onadd}<Button type="button" variant="outline" size="sm" onclick={onadd}>Add Account</Button>{/if}
			<Button type="button" variant="default" size="sm" onclick={onclose}>Close</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>

<style>
	.guide-steps { list-style: none; counter-reset: step; padding: 0; margin: 0 0 var(--space-md); display: flex; flex-direction: column; gap: var(--space-md); }
	.guide-steps > li { counter-increment: step; position: relative; padding-left: 2rem; font-size: var(--text-sm); }
	.guide-steps > li::before { content: counter(step); position: absolute; left: 0; top: 0; width: 1.4rem; height: 1.4rem; border-radius: 50%; background: var(--color-accent); color: var(--color-on-accent, #fff); font-size: var(--text-xs); display: inline-flex; align-items: center; justify-content: center; }
	.guide-steps h3, .guide-sub { font-size: var(--text-sm); font-weight: 600; margin: 0 0 var(--space-xs); }
	.guide-steps p, .guide-steps ul { margin: 0 0 var(--space-xs); color: var(--color-muted); }
	.guide-steps ul { padding-left: 1.1rem; }
	.guide-path { color: inherit !important; }
	.guide-warn { padding: var(--space-xs) var(--space-sm); border: var(--rule-default); border-radius: var(--radius-md); background: var(--surface-raised); }
	.guide-copy { display: flex; align-items: center; gap: var(--space-xs); margin-bottom: var(--space-xs); }
	.guide-copy code { flex: 1; min-width: 0; overflow-wrap: anywhere; padding: var(--space-xs) var(--space-sm); background: var(--surface-raised); border: var(--rule-default); border-radius: var(--radius-md); font-size: var(--text-xs); }
	.guide-scopes { display: flex; flex-wrap: wrap; gap: var(--space-xs); }
	.guide-scopes code { font-size: var(--text-xs); padding: 0 var(--space-xs); border: var(--rule-default); border-radius: var(--radius-sm); }
	.guide-link { background: none; border: 0; padding: 0; color: var(--color-accent); cursor: pointer; text-decoration: underline; font: inherit; }
	.guide-table { width: 100%; border-collapse: collapse; font-size: var(--text-xs); margin-bottom: var(--space-md); }
	.guide-table th, .guide-table td { text-align: left; padding: var(--space-xs); border-bottom: var(--rule-default); vertical-align: top; }
	code { font-size: 0.92em; }
	a { color: var(--color-accent); text-decoration: underline; }
</style>
