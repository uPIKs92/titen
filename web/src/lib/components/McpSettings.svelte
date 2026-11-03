<script lang="ts">
	import {
		mcpRequest,
		listAgentKeys,
		createAgentKey,
		revokeAgentKey,
		ApiError,
		type McpTool,
		type AgentKey,
	} from '$lib/api';
	import { formatDateShort } from '$lib/tz';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import { Input } from '$lib/components/ui/input';
	import { freshAgentKey } from '$lib/freshAgentKey.svelte';
	import { toast } from '$lib/toast.svelte';
	import { Button } from '$lib/components/ui/button';

	type Format = 'json' | 'claude' | 'curl' | 'stdio';

	const endpoint = $derived(
		(typeof window !== 'undefined' ? window.location.origin : 'https://your-titen-host') + '/api/mcp'
	);
	let format = $state<Format>('json');
	let copied = $state('');

	let testing = $state(false);
	let result = $state<{ version: string; protocol: string; tools: McpTool[] } | null>(null);
	let error = $state('');

	const newKey = $derived(freshAgentKey.current);
	// Config snippets use the key just created; otherwise a placeholder.
	const keyText = $derived(newKey?.value ?? '<TITEN_API_KEY>');

	const snippets = $derived<Record<Format, { label: string; hint: string; code: string }>>({
		json: {
			label: 'MCP client (JSON)',
			hint: 'Hermes, Cursor, Claude Desktop and other clients that take an mcpServers block.',
			code: JSON.stringify(
				{ mcpServers: { titen: { url: endpoint, headers: { 'X-API-Key': keyText } } } },
				null,
				2
			),
		},
		claude: {
			label: 'Claude Code',
			hint: 'Run once in a terminal.',
			code: `claude mcp add --transport http titen ${endpoint} \\\n  --header "X-API-Key: ${keyText}"`,
		},
		curl: {
			label: 'curl',
			hint: 'Quick check from any machine that can reach this server.',
			code: `curl -s -X POST ${endpoint} \\\n  -H "X-API-Key: ${keyText}" -H "content-type: application/json" \\\n  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'`,
		},
		stdio: {
			label: 'Local (stdio)',
			hint: 'Only when the agent runs on the same machine. Set TITEN_ENCRYPTION_KEY if tokens are encrypted.',
			code: JSON.stringify(
				{
					mcpServers: {
						titen: {
							command: '/path/to/titen-mcp',
							env: { TITEN_DB_PATH: '/path/to/titen.db', TITEN_ENCRYPTION_KEY: '<same key as the server>' },
						},
					},
				},
				null,
				2
			),
		},
	});

	const kind = (t: McpTool) =>
		t.annotations?.readOnlyHint ? 'read' : t.annotations?.destructiveHint ? 'destructive' : 'write';
	const groups = $derived.by(() => {
		const order = [
			['read', 'Read-only'],
			['write', 'Writes / publishes'],
			['destructive', 'Destructive'],
		] as const;
		return order
			.map(([k, label]) => ({ k, label, tools: (result?.tools ?? []).filter((t) => kind(t) === k) }))
			.filter((g) => g.tools.length > 0);
	});

	async function copy(text: string, id: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = id;
			setTimeout(() => (copied = ''), 1500);
		} catch {
			toast('Copy failed — select the text and copy manually', 'error');
		}
	}

	// ── Agent keys ──
	let keys = $state<AgentKey[]>([]);
	let keysLoaded = $state(false);
	let keyName = $state('');
	let keyScope = $state<'read' | 'write'>('read');
	let creating = $state(false);
	let revoking = $state<AgentKey | null>(null);
	const activeKeys = $derived(keys.filter((k) => !k.revoked_at));

	async function loadKeys() {
		try {
			keys = await listAgentKeys();
		} catch {
			toast('Failed to load agent keys', 'error');
		} finally {
			keysLoaded = true;
		}
	}

	async function submitKey() {
		if (!keyName.trim() || creating) return;
		creating = true;
		try {
			const r = await createAgentKey({ name: keyName.trim(), scope: keyScope });
			freshAgentKey.set({ name: r.key.name, value: r.api_key });
			keyName = '';
			await loadKeys();
		} catch (e) {
			toast(e instanceof ApiError ? e.message : 'Failed to create key', 'error');
		} finally {
			creating = false;
		}
	}

	async function confirmRevoke() {
		const k = revoking;
		revoking = null;
		if (!k) return;
		try {
			await revokeAgentKey(k.id);
			toast(`Revoked "${k.name}"`, 'success');
			await loadKeys();
		} catch {
			toast('Failed to revoke key', 'error');
		}
	}

	$effect(() => {
		if (!keysLoaded) loadKeys();
	});

	async function test() {
		testing = true;
		error = '';
		try {
			const init = await mcpRequest<{ protocolVersion: string; serverInfo: { version: string } }>(
				'initialize',
				{ protocolVersion: '2025-06-18' }
			);
			const list = await mcpRequest<{ tools: McpTool[] }>('tools/list');
			result = { version: init.serverInfo.version, protocol: init.protocolVersion, tools: list.tools };
		} catch (e) {
			result = null;
			error = e instanceof ApiError ? e.message : 'Could not reach the MCP endpoint';
		} finally {
			testing = false;
		}
	}

	$effect(() => {
		if (!result && !error && !testing) test();
	});
</script>

<section class="mcp-section">
	<h2 class="mcp-title">Agent integration (MCP)</h2>
	<div class="mcp-card">
		<p class="mcp-text">
			Let AI agents (Hermes, Claude, Cursor, …) manage accounts, posts, schedules and analytics through the
			Model Context Protocol. Agents authenticate with this server's API key — the
			<code>TITEN_API_KEY</code> value from its environment. It is never shown here.
		</p>

		<div class="mcp-field">
			<span class="mcp-label">Endpoint</span>
			<div class="mcp-row">
				<code class="mcp-code-inline" data-testid="mcp-endpoint">{endpoint}</code>
				<Button type="button" variant="outline" size="sm" onclick={() => copy(endpoint, 'endpoint')}>
					{copied === 'endpoint' ? 'Copied' : 'Copy'}
				</Button>
			</div>
			<p class="mcp-hint">Use the address your agent can reach (a public HTTPS URL for remote agents). Header: <code>X-API-Key</code>.</p>
		</div>

		<div class="mcp-field">
			<div class="mcp-seg" role="tablist" aria-label="Config format">
				{#each Object.entries(snippets) as [key, s] (key)}
					<button type="button" role="tab" class="mcp-seg-btn" class:active={format === key} aria-selected={format === key} onclick={() => (format = key as Format)}>{s.label}</button>
				{/each}
			</div>
			<p class="mcp-hint">{snippets[format].hint}</p>
			<div class="mcp-snippet">
				<pre data-testid="mcp-snippet"><code>{snippets[format].code}</code></pre>
				<Button type="button" variant="outline" size="sm" class="mcp-copy" onclick={() => copy(snippets[format].code, 'snippet')}>
					{copied === 'snippet' ? 'Copied' : 'Copy'}
				</Button>
			</div>
			{#if newKey}
				<p class="mcp-hint" data-testid="mcp-snippet-note">These snippets include the key you just created (“{newKey.name}”). Copy them before leaving this page — the key can’t be shown again.</p>
			{:else}
				<p class="mcp-hint">Replace <code>&lt;TITEN_API_KEY&gt;</code> with an agent key from below (or the master key). Creating a key fills it in here.</p>
			{/if}
		</div>
	</div>

	<h2 class="mcp-title">Agent keys</h2>
	<div class="mcp-card">
		<p class="mcp-text">
			Give each agent its own key. <strong>Read</strong> keys can only read data and call read-only tools;
			<strong>Write</strong> keys can also publish, schedule and connect accounts. Neither can manage keys or
			settings. Revoke a key at any time without affecting the others.
		</p>

		{#if newKey}
			<div class="mcp-newkey" role="status" data-testid="mcp-newkey">
				<p class="mcp-text"><strong>Key for “{newKey.name}”</strong> — copy it now, it won’t be shown again. It is filled into the snippets below.</p>
				<div class="mcp-row">
					<code class="mcp-code-inline" data-testid="mcp-newkey-value">{newKey.value}</code>
					<Button type="button" size="sm" onclick={() => newKey && copy(newKey.value, 'newkey')}>{copied === 'newkey' ? 'Copied' : 'Copy'}</Button>
					<Button type="button" variant="ghost" size="sm" onclick={() => freshAgentKey.clear()}>Done</Button>
				</div>
			</div>
		{/if}

		<form class="mcp-keyform" onsubmit={(e) => { e.preventDefault(); submitKey(); }}>
			<div class="mcp-keyform-name">
				<label class="mcp-label" for="agent_key_name">Name</label>
				<Input id="agent_key_name" type="text" bind:value={keyName} placeholder="e.g. Hermes (analytics)" maxlength={80} disabled={creating} />
			</div>
			<div>
				<label class="mcp-label" for="agent_key_scope">Access</label>
				<select id="agent_key_scope" class="form-select" bind:value={keyScope} disabled={creating}>
					<option value="read">Read-only</option>
					<option value="write">Read &amp; write</option>
				</select>
			</div>
			<Button type="submit" disabled={!keyName.trim() || creating}>{creating ? 'Creating…' : 'Create key'}</Button>
		</form>

		{#if activeKeys.length > 0}
			<ul class="mcp-keys" data-testid="mcp-keys">
				{#each activeKeys as k (k.id)}
					<li class="mcp-key">
						<div class="mcp-key-main">
							<strong>{k.name}</strong>
							<span class="mcp-scope mcp-scope--{k.scope}">{k.scope === 'read' ? 'Read-only' : 'Read & write'}</span>
						</div>
						<div class="mcp-key-meta">
							<code>{k.key_prefix}…</code>
							<span>Created {formatDateShort(k.created_at)}</span>
							<span>{k.last_used_at ? `Last used ${formatDateShort(k.last_used_at)}` : 'Never used'}</span>
						</div>
						<Button variant="ghost" size="sm" onclick={() => (revoking = k)}>Revoke</Button>
					</li>
				{/each}
			</ul>
		{:else if keysLoaded}
			<p class="mcp-hint">No agent keys yet. Until you create one, agents would have to use the master <code>TITEN_API_KEY</code>.</p>
		{/if}
	</div>

	<div class="mcp-head">
		<h2 class="mcp-title">Connection &amp; tools</h2>
		<Button variant="outline" size="sm" onclick={test} disabled={testing}>{testing ? 'Testing…' : 'Test connection'}</Button>
	</div>
	<div class="mcp-card">
		{#if error}
			<p class="mcp-error" role="alert" data-testid="mcp-error">{error}</p>
		{:else if result}
			<p class="mcp-text" data-testid="mcp-status">
				<span class="mcp-ok">Connected</span> · Titen {result.version} · protocol {result.protocol} · {result.tools.length} tools
			</p>
			{#each groups as g (g.k)}
				<details class="mcp-group" data-testid="mcp-group-{g.k}">
					<summary class="mcp-group-summary">
						<span class="mcp-group-title">{g.label}</span>
						<span class="mcp-count">{g.tools.length}</span>
					</summary>
					<ul class="mcp-tools">
						{#each g.tools as t (t.name)}
							<li><code>{t.name}</code><span class="mcp-desc">{t.description}</span></li>
						{/each}
					</ul>
				</details>
			{/each}
		{:else}
			<p class="mcp-hint">Checking…</p>
		{/if}
	</div>
</section>

<ConfirmDialog
	open={!!revoking}
	title="Revoke key?"
	message={revoking ? `“${revoking.name}” will stop working immediately. This cannot be undone.` : ''}
	confirmLabel="Revoke"
	onconfirm={confirmRevoke}
	oncancel={() => (revoking = null)}
/>

<style>
	.mcp-section { margin-bottom: var(--space-xl); }
	.mcp-title { font-size: var(--text-md); font-weight: 600; margin: var(--space-lg) 0 var(--space-sm); }
	.mcp-section > .mcp-title:first-child { margin-top: 0; }
	.mcp-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-sm); flex-wrap: wrap; margin-top: var(--space-lg); }
	.mcp-head .mcp-title { margin: 0 0 var(--space-sm); }
	.mcp-card { background: var(--card); border: var(--rule-default); border-radius: var(--plk-radius-lg); padding: var(--space-lg); display: flex; flex-direction: column; gap: var(--space-md); }
	.mcp-text { font-size: var(--text-sm); color: var(--muted-foreground); line-height: 1.6; }
	.mcp-hint { font-size: var(--text-xs); color: var(--muted-foreground); margin-top: var(--space-2xs); }
	.mcp-label { font-family: var(--plk-font-mono); font-size: var(--text-xs); color: var(--muted-foreground); text-transform: uppercase; letter-spacing: 0.08em; }
	.mcp-field { display: flex; flex-direction: column; gap: var(--space-2xs); }
	.mcp-row { display: flex; gap: var(--space-xs); align-items: center; flex-wrap: wrap; }
	.mcp-code-inline { font-family: var(--plk-font-mono); font-size: var(--text-sm); word-break: break-all; flex: 1 1 14rem; min-width: 0; }
	.mcp-seg { display: flex; flex-wrap: wrap; gap: var(--space-2xs); }
	.mcp-seg-btn { font-size: var(--text-sm); padding: 0.25rem 0.75rem; border: var(--rule-default); border-radius: var(--plk-radius-md); background: transparent; color: var(--muted-foreground); cursor: pointer; min-height: 2rem; }
	.mcp-seg-btn.active { color: var(--foreground); border-color: var(--primary-text); background: color-mix(in srgb, var(--primary-text) 10%, transparent); }
	.mcp-snippet { position: relative; }
	.mcp-snippet pre { margin: 0; padding: var(--space-md); padding-right: 5.5rem; border: var(--rule-default); border-radius: var(--plk-radius-md); overflow-x: auto; font-family: var(--plk-font-mono); font-size: var(--text-xs); line-height: 1.6; }
	.mcp-snippet :global(.mcp-copy) { position: absolute; top: var(--space-xs); right: var(--space-xs); }
	.mcp-ok { color: var(--success-ink); font-weight: 600; }
	.mcp-error { color: var(--destructive-ink); font-size: var(--text-sm); }
	.mcp-group { border: var(--rule-default); border-radius: var(--plk-radius-md); }
	.mcp-group-summary { display: flex; align-items: center; gap: var(--space-xs); padding: var(--space-xs) var(--space-sm); min-height: 2.25rem; cursor: pointer; }
	.mcp-group[open] > .mcp-group-summary { border-bottom: var(--rule-default); }
	.mcp-group .mcp-tools { padding: var(--space-sm); }
	.mcp-group-title { font-size: var(--text-sm); font-weight: 600; }
	.mcp-newkey { border: 1px solid var(--primary); border-radius: var(--plk-radius-md); padding: var(--space-md); display: flex; flex-direction: column; gap: var(--space-xs); }
	.mcp-keyform { display: flex; gap: var(--space-sm); align-items: flex-end; flex-wrap: wrap; }
	.mcp-keyform-name { flex: 1 1 14rem; min-width: 0; }
	.mcp-keys { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--space-xs); }
	.mcp-key { display: flex; gap: var(--space-sm); align-items: center; justify-content: space-between; flex-wrap: wrap; border: var(--rule-default); border-radius: var(--plk-radius-md); padding: var(--space-xs) var(--space-sm); }
	.mcp-key-main { display: flex; gap: var(--space-xs); align-items: center; flex-wrap: wrap; }
	.mcp-key-meta { display: flex; gap: var(--space-sm); flex-wrap: wrap; font-size: var(--text-xs); color: var(--muted-foreground); flex: 1 1 14rem; }
	.mcp-scope { font-size: var(--text-xs); padding: 0 0.5rem; border-radius: 999px; border: var(--rule-default); }
	.mcp-scope--write { color: var(--warning-ink); background: color-mix(in oklch, var(--warning) 18%, transparent); }
	.mcp-count { color: var(--muted-foreground); font-weight: 400; margin-left: var(--space-2xs); }
	.mcp-tools { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--space-2xs); }
	.mcp-tools li { display: flex; gap: var(--space-sm); align-items: baseline; flex-wrap: wrap; font-size: var(--text-sm); }
	.mcp-tools code { font-family: var(--plk-font-mono); font-size: var(--text-xs); flex: 0 0 auto; }
	.mcp-desc { color: var(--muted-foreground); flex: 1 1 16rem; min-width: 0; }
	@media (max-width: 48rem) { .mcp-card { padding: var(--space-md); } }
</style>
