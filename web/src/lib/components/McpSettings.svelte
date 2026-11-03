<script lang="ts">
	import { mcpRequest, ApiError, type McpTool } from '$lib/api';
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

	const snippets = $derived<Record<Format, { label: string; hint: string; code: string }>>({
		json: {
			label: 'MCP client (JSON)',
			hint: 'Hermes, Cursor, Claude Desktop and other clients that take an mcpServers block.',
			code: JSON.stringify(
				{ mcpServers: { titen: { url: endpoint, headers: { 'X-API-Key': '<TITEN_API_KEY>' } } } },
				null,
				2
			),
		},
		claude: {
			label: 'Claude Code',
			hint: 'Run once in a terminal.',
			code: `claude mcp add --transport http titen ${endpoint} \\\n  --header "X-API-Key: <TITEN_API_KEY>"`,
		},
		curl: {
			label: 'curl',
			hint: 'Quick check from any machine that can reach this server.',
			code: `curl -s -X POST ${endpoint} \\\n  -H "X-API-Key: <TITEN_API_KEY>" -H "content-type: application/json" \\\n  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'`,
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
			<p class="mcp-hint">Replace <code>&lt;TITEN_API_KEY&gt;</code> with your key.</p>
		</div>
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
				<div class="mcp-group">
					<h3 class="mcp-group-title">{g.label} <span class="mcp-count">{g.tools.length}</span></h3>
					<ul class="mcp-tools">
						{#each g.tools as t (t.name)}
							<li><code>{t.name}</code><span class="mcp-desc">{t.description}</span></li>
						{/each}
					</ul>
				</div>
			{/each}
		{:else}
			<p class="mcp-hint">Checking…</p>
		{/if}
	</div>
</section>

<style>
	.mcp-section { margin-bottom: var(--space-xl); }
	.mcp-title { font-size: var(--text-md); font-weight: 600; margin: var(--space-lg) 0 var(--space-sm); }
	.mcp-section > .mcp-title:first-child { margin-top: 0; }
	.mcp-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-sm); flex-wrap: wrap; margin-top: var(--space-lg); }
	.mcp-head .mcp-title { margin: 0 0 var(--space-sm); }
	.mcp-card { background: var(--surface-raised); border: var(--rule-default); border-radius: var(--radius-lg); padding: var(--space-lg); display: flex; flex-direction: column; gap: var(--space-md); }
	.mcp-text { font-size: var(--text-sm); color: var(--color-muted); line-height: 1.6; }
	.mcp-hint { font-size: var(--text-xs); color: var(--color-muted); margin-top: var(--space-2xs); }
	.mcp-label { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--color-muted); text-transform: uppercase; letter-spacing: 0.08em; }
	.mcp-field { display: flex; flex-direction: column; gap: var(--space-2xs); }
	.mcp-row { display: flex; gap: var(--space-xs); align-items: center; flex-wrap: wrap; }
	.mcp-code-inline { font-family: var(--font-mono); font-size: var(--text-sm); word-break: break-all; flex: 1 1 14rem; min-width: 0; }
	.mcp-seg { display: flex; flex-wrap: wrap; gap: var(--space-2xs); }
	.mcp-seg-btn { font-size: var(--text-sm); padding: 0.25rem 0.75rem; border: var(--rule-default); border-radius: var(--radius-md); background: transparent; color: var(--color-muted); cursor: pointer; min-height: 2rem; }
	.mcp-seg-btn.active { color: var(--color-text, inherit); border-color: var(--color-accent); background: color-mix(in srgb, var(--color-accent) 10%, transparent); }
	.mcp-snippet { position: relative; }
	.mcp-snippet pre { margin: 0; padding: var(--space-md); padding-right: 5.5rem; border: var(--rule-default); border-radius: var(--radius-md); overflow-x: auto; font-family: var(--font-mono); font-size: var(--text-xs); line-height: 1.6; }
	.mcp-snippet :global(.mcp-copy) { position: absolute; top: var(--space-xs); right: var(--space-xs); }
	.mcp-ok { color: var(--color-success, #16a34a); font-weight: 600; }
	.mcp-error { color: var(--color-error); font-size: var(--text-sm); }
	.mcp-group-title { font-size: var(--text-sm); font-weight: 600; margin-bottom: var(--space-2xs); }
	.mcp-count { color: var(--color-muted); font-weight: 400; margin-left: var(--space-2xs); }
	.mcp-tools { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--space-2xs); }
	.mcp-tools li { display: flex; gap: var(--space-sm); align-items: baseline; flex-wrap: wrap; font-size: var(--text-sm); }
	.mcp-tools code { font-family: var(--font-mono); font-size: var(--text-xs); flex: 0 0 auto; }
	.mcp-desc { color: var(--color-muted); flex: 1 1 16rem; min-width: 0; }
	@media (max-width: 48rem) { .mcp-card { padding: var(--space-md); } }
</style>
