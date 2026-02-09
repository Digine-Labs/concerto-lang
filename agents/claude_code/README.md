# claude_code host middleware

Reference host project that bridges Concerto host protocol to Claude Code CLI.

It translates:

- Concerto prompt input (`stdin` line) -> Claude Code invocation
- Claude CLI output -> Concerto NDJSON messages (`progress`, `partial`, `result`, `error`)
- Concerto `response` lines -> middleware decisions for `question` / `approval`

## Why this exists

Concerto hosts are protocol adapters. This project demonstrates a practical adapter layer for Claude Code without changing Concerto runtime internals.

## Files

- `claude_code_host.py`: middleware process used by `[hosts.*]` manifest config
- `Concerto.toml.example`: sample host connector config
- `main.conc.example`: sample `listen` program using this host

## Quick smoke tests

One-shot mode (safe default for `Host.execute()`):

```bash
printf 'Write a Rust function that reverses a string\n' \
  | python3 hosts/claude_code/claude_code_host.py --mode oneshot --mock
```

Streaming mode with supervisor responses (`listen` workflows):

```bash
cat <<'EOF' \
  | python3 hosts/claude_code/claude_code_host.py --mode stream --interactive --mock
Delete old migration files and regenerate schema
{"type":"response","in_reply_to":"question","value":"Run full tests and keep a rollback plan."}
{"type":"response","in_reply_to":"approval","value":"yes"}
EOF
```

## Concerto integration

Use the middleware as a host command in `Concerto.toml`:

```toml
[hosts.claude_code]
transport = "stdio"
command = "python3"
args = [
  "hosts/claude_code/claude_code_host.py",
  "--mode", "stream",
  "--interactive",
  "--claude-command", "claude",
  "--claude-args", "--print --output-format stream-json",
  "--prompt-mode", "arg-last"
]
timeout = 900
```

Then reference it from Concerto source:

```concerto
host ClaudeCode {
    connector: "claude_code",
    output_format: "json",
    timeout: 900,
}

fn main() {
    let result = listen ClaudeCode.execute("Refactor auth module and update tests") {
        "progress" => |m| { emit("host:progress", m); },
        "partial" => |chunk| { emit("host:partial", chunk); },
        "question" => |q| { "Prefer safe incremental edits with tests." },
        "approval" => |req| { "yes" },
    };
    emit("host:result", result);
}
```

## Configuration

All options can be passed as CLI args (recommended for `Concerto.toml`) or env vars.

| Option | Env | Default | Purpose |
|---|---|---|---|
| `--mode` | `CONCERTO_HOST_MODE` | `oneshot` | `oneshot` for single-line result, `stream` for `listen` flows |
| `--interactive` | `CONCERTO_HOST_INTERACTIVE` | `false` | Emit `question`/`approval` and wait for Concerto responses |
| `--mock` | `CONCERTO_HOST_MOCK` | `false` | Use built-in deterministic mock output |
| `--claude-command` | `CLAUDE_CODE_COMMAND` | `claude` | Claude CLI executable |
| `--claude-args` | `CLAUDE_CODE_ARGS` | `--print` | Args string for Claude command |
| `--prompt-mode` | `CLAUDE_CODE_PROMPT_MODE` | `arg-last` | Prompt injection style: `arg-last`, `stdin`, `json-stdin` |
| `--timeout-secs` | `CLAUDE_CODE_TIMEOUT_SECS` | `600` | Claude invocation timeout |
| `--response-timeout-secs` | `CONCERTO_HOST_RESPONSE_TIMEOUT_SECS` | `30` | Wait time for `question`/`approval` responses |
| `--history-turns` | `CONCERTO_HOST_HISTORY_TURNS` | `3` | Number of recent sessions folded into next prompt |
| `--max-partials` | `CONCERTO_HOST_MAX_PARTIALS` | `32` | Cap on emitted `partial` messages per session |

## Notes

- `oneshot` mode is the safest default because `Host.execute()` reads only one output line.
- `stream` mode is intended for `listen` expressions.
- `interactive` mode can pause waiting for responses; keep handlers for `question` and `approval` in your `listen` block.
- Claude output schemas can vary across versions; this middleware extracts text with best-effort heuristics.
