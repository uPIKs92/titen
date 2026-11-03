//! Wire-protocol tests: spawn the real `titen-mcp` binary and speak JSON-RPC over stdio.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(lines: &[&str]) -> Vec<serde_json::Value> {
    let db = std::env::temp_dir().join(format!("titen-mcp-proto-{}.db", uuid::Uuid::now_v7()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_titen-mcp"))
        .env("TITEN_DB_PATH", &db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn titen-mcp");
    {
        let mut stdin = child.stdin.take().unwrap();
        for l in lines {
            writeln!(stdin, "{l}").unwrap();
        }
    } // close stdin → server exits after draining
    let out = child.wait_with_output().unwrap();
    let _ = std::fs::remove_file(&db);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each stdout line is JSON"))
        .collect()
}

#[test]
fn notifications_get_no_response() {
    let r = run(&[
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    ]);
    assert_eq!(r.len(), 1, "only the ping is answered: {r:?}");
    assert_eq!(r[0]["id"], 1);
    assert_eq!(r[0]["result"], serde_json::json!({}));
}

#[test]
fn errors_are_top_level_jsonrpc_errors() {
    let r = run(&[
        r#"{"jsonrpc":"2.0","id":"abc","method":"nope"}"#,
        r#"{not json"#,
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{}}"#,
    ]);
    assert_eq!(r[0]["id"], "abc");
    assert_eq!(r[0]["error"]["code"], -32601);
    assert!(r[0].get("result").is_none());
    assert_eq!(r[1]["id"], serde_json::Value::Null);
    assert_eq!(r[1]["error"]["code"], -32700);
    assert_eq!(r[2]["error"]["code"], -32602);
}

#[test]
fn initialize_negotiates_protocol_version() {
    let init = |v: &str| {
        run(&[&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"{v}"}}}}"#
        )])
        .remove(0)
    };
    assert_eq!(init("2025-03-26")["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(init("1999-01-01")["result"]["protocolVersion"], "2025-06-18");
    assert!(init("2024-11-05")["result"]["instructions"].is_string());
}

#[test]
fn tools_carry_safety_annotations() {
    let r = run(&[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
    let tools = r[0]["result"]["tools"].as_array().unwrap();
    let get = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap()["annotations"].clone();
    assert_eq!(get("list_posts")["readOnlyHint"], true);
    assert_eq!(get("delete_post")["destructiveHint"], true);
    assert_eq!(get("create_post")["openWorldHint"], true);
    assert_eq!(get("create_post")["readOnlyHint"], false);
}
