//! `titen-mcp` — stdio transport for the Titen MCP server.
//!
//! All protocol and tool logic lives in the `titen_mcp` library so the HTTP
//! transport (`POST /api/mcp` in `titen-api`) behaves identically.
//!
//! Set `TITEN_DB_PATH` to point to the SQLite database. If the database holds
//! encrypted tokens, also set `TITEN_ENCRYPTION_KEY`.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use titen_core::{Store, ThreadsClient};

fn main() {
    eprintln!("titen-mcp starting (stdio JSON-RPC)");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    let db_path = titen_core::config::default_db_path();
    titen_core::config::ensure_parent_dir(&db_path);
    let (store, threads_client) = rt.block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{db_path}?mode=rwc"))
            .await
            .expect("Failed to connect to database");
        let store = Arc::new(Store::new(pool));
        store.migrate().await.expect("Failed to run migrations");
        let client = Arc::new(ThreadsClient::new(store.clone()));
        (store, client)
    });

    let mut stdout = io::stdout();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(req) => rt.block_on(titen_mcp::handle_message(&store, &threads_client, &req)),
            Err(e) => Some(titen_mcp::rpc_error(
                serde_json::Value::Null,
                -32700,
                &format!("Parse error: {e}"),
            )),
        };
        if let Some(resp) = response {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap_or_default());
            let _ = stdout.flush();
        }
    }
}
