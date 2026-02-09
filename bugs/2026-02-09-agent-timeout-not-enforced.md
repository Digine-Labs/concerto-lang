# Bug Report: Agent `timeout` Is Not Enforced for `execute` or `listen`

## Status: OPEN (2026-02-09)

## Summary
Agent timeout values from manifest/IR are parsed and stored, but never used during blocking reads. Both `Agent.execute(...)` and `listen ...` can block until subprocess output/exit, ignoring configured timeout.

## Severity
High (can hang language runtime workflows and CI runs).

## Date
2026-02-09

## Affected Components
- `crates/concerto-runtime/src/agent.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A (`execute`)
Manifest excerpt:

```toml
[agents.sleeper]
transport = "stdio"
command = "bash"
args = ["-lc", "sleep 3; echo '{\"text\":\"late\"}'"]
timeout = 1
```

Source:

```concerto
agent Sleeper {
    connector: "sleeper",
    output_format: "json",
    timeout: 1,
}

fn main() {
    let result = Sleeper.execute("ping");
    emit("result", result);
}
```

Command:

```bash
/usr/bin/time -f 'ELAPSED_SEC=%e' timeout 8s cargo run -q -p concerto -- run /tmp/concerto-audit/bug_agent_timeout_execute/src/main.conc
```

Observed output:

- `[emit:result] Ok(late)`
- `ELAPSED_SEC=3.12` (greater than configured timeout `1`)

## Reproduction B (`listen`)
Manifest excerpt:

```toml
[agents.silent]
transport = "stdio"
command = "bash"
args = ["-lc", "sleep 5"]
timeout = 1
```

Observed runtime output after ~5s:

- `[emit:listen:start] ...`
- `[emit:listen:error] ... exited without sending a result`
- `ELAPSED_SEC=5.10`

Expected behavior: timeout should trigger around 1 second.

## Root Cause
- `timeout: Duration` is stored in `AgentClient` but not consulted in blocking IO paths.
- `AgentClient::execute()` uses blocking `read_line` without timeout handling.
- `AgentClient::read_message()` (used by `listen`) also uses blocking `read_line` with no timeout handling.
- `VM::run_listen_loop()` repeatedly calls `read_message()` and inherits this indefinite/blocking behavior.

## Impact
- `host_streaming`/agent integrations can hang.
- Runtime-level timeout configuration is misleading.
- Long-running agent sessions can stall orchestration pipelines.

## Workaround
Wrap CLI invocations with external process timeout (shell-level `timeout`) until runtime enforces agent timeouts internally.

## Suggested Fix
Add internal timeout enforcement for agent reads/writes (execute and streaming), and surface timeout as a structured runtime error. Add integration tests that verify timeout expiry for both `execute` and `listen`.
