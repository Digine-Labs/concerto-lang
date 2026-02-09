# Host Initialization Arguments

## Problem

Hosts are **middleware applications** — typically TypeScript or Python projects — that sit between Concerto and external tools/agents. Concerto spawns the middleware process and communicates with it over stdio. The middleware then internally manages whatever it wraps (Claude Code, a custom LLM pipeline, a code analysis tool, etc.).

```
Concerto Runtime  <--stdio-->  Host Middleware (TS/Python)  --->  Claude Code / LLM / Tool
```

Concerto does NOT run Claude Code directly. The host middleware runs it. This is a critical distinction: the `command` in TOML points to the middleware project, not to the end tool.

Currently, the only way to configure a host is through raw CLI `args`:

```toml
[hosts.claude_code_host]
transport = "stdio"
command = "npx"
args = ["ts-node", "hosts/claude-code-host/index.ts"]
```

But the middleware often needs structured configuration — like which directory Claude Code should work in, which model to use, or what permissions to grant. These are middleware-level concerns that don't belong in the Concerto source.

## Proposal

Add a `[hosts.<name>.params]` table in `Concerto.toml` for named arguments passed to the host middleware at initialization.

```toml
# A TypeScript middleware that internally manages Claude Code
[hosts.claude_code_host]
transport = "stdio"
command = "npx"
args = ["ts-node", "hosts/claude-code-host/index.ts"]
timeout = 600

[hosts.claude_code_host.params]
work_dir = "/home/user/my-project"
model = "opus"
allowed_tools = ["read", "write", "bash"]

# A Python middleware that wraps a custom LLM pipeline
[hosts.researcher]
transport = "stdio"
command = "python"
args = ["hosts/researcher/main.py"]
timeout = 300

[hosts.researcher.params]
model = "gpt-4o"
api_endpoint = "https://api.openai.com/v1"
max_tokens = 4096
temperature = 0.7
search_provider = "tavily"
```

## How It Works

1. **Defined in TOML only** — the `.conc` source does not change. The host declaration stays purely structural:
   ```concerto
   host ClaudeCodeHost {
       connector: "claude_code_host",
       output_format: "json",
   }
   ```

2. **Passed to middleware at spawn time** — when the runtime spawns the middleware process, it sends an initialization message (JSON) containing the params before any prompts:
   ```json
   {"type": "init", "params": {"work_dir": "/home/user/my-project", "model": "opus", "allowed_tools": ["read", "write", "bash"]}}
   ```

3. **Middleware uses params to configure itself** — the middleware reads the init message and sets up its internal tools accordingly:

   ```typescript
   // hosts/claude-code-host/index.ts
   import { spawn } from 'child_process';

   const initMsg = JSON.parse(await readLine());
   const { work_dir, model, allowed_tools } = initMsg.params;

   // Middleware spawns Claude Code with the configured params
   const claude = spawn('claude', [
     '--output-format', 'stream-json',
     '--model', model,
     '--allowedTools', allowed_tools.join(','),
   ], { cwd: work_dir });

   // Middleware handles stdio relay, message translation, etc.
   ```

   ```python
   # hosts/researcher/main.py
   import json, sys

   init_msg = json.loads(sys.stdin.readline())
   model = init_msg["params"]["model"]
   api_endpoint = init_msg["params"]["api_endpoint"]

   # Middleware sets up its LLM client with the configured params
   client = OpenAI(base_url=api_endpoint)
   ```

4. **Embedded into IR at compile time** — the compiler already embeds host TOML config into the IR. The `params` table would be added to `IrHost` as a `HashMap<String, serde_json::Value>`.

## Architecture Diagram

```
                    Concerto.toml
                    ┌─────────────────────────┐
                    │ [hosts.claude_code_host] │
                    │ command = "npx"          │
                    │ args = ["ts-node", ...]  │
                    │                          │
                    │ [hosts...params]         │
                    │ work_dir = "/project"    │
                    │ model = "opus"           │
                    └────────────┬─────────────┘
                                 │ embedded at compile time
                                 v
┌──────────────┐    ┌──────────────────────┐    ┌─────────────────┐
│  .conc source │───>│   Concerto Runtime    │───>│  Host Middleware │
│              │    │                      │    │  (TS/Python)    │
│  host X {    │    │  spawns middleware    │    │                 │
│    connector │    │  sends init + params  │    │  receives params│
│  }           │    │  relays prompts      │    │  spawns Claude  │
│              │    │  reads NDJSON        │    │  translates msgs│
└──────────────┘    └──────────────────────┘    └─────────────────┘
                                                        │
                                                        v
                                                ┌─────────────────┐
                                                │  Claude Code /  │
                                                │  LLM / Tool     │
                                                └─────────────────┘
```

## Wire Protocol Addition

The init message is the **first message** sent to the middleware after spawning, before any `execute()` prompts:

```
--> {"type": "init", "params": {"work_dir": "/path", "model": "opus"}}
<-- {"type": "init_ack"}
--> {"type": "prompt", "text": "Refactor the auth module"}
<-- {"type": "progress", "message": "Analyzing codebase...", "percent": 10}
<-- {"type": "result", "text": "Done. Refactored 3 files."}
```

If the middleware does not require params (empty `[params]` or no `[params]` table), the runtime skips the init message for backwards compatibility.

## Why TOML, Not Source

- **Deployment-specific**: Working directories, models, API endpoints vary per environment — they belong in config.
- **Middleware concern**: The `.conc` source defines the agent orchestration. How the middleware internally spawns Claude Code is the middleware's business.
- **No language complexity**: No constructor syntax or parameterized types needed in the grammar.
- **Consistent with connections**: LLM provider config (`api_key_env`, `default_model`) already lives in TOML.

## Use Cases

| Host Middleware | Param | Purpose |
|----------------|-------|---------|
| Claude Code Host | `work_dir` | Directory where Claude Code operates |
| Claude Code Host | `model` | Which Claude model the middleware passes to Claude |
| Claude Code Host | `allowed_tools` | Tool permissions for the Claude session |
| Python Researcher | `model` | LLM model name for the research pipeline |
| Python Researcher | `api_endpoint` | Custom API endpoint |
| Python Researcher | `search_provider` | Which search API to use (tavily, serper, etc.) |
| TS Code Reviewer | `repo_path` | Repository to analyze |
| TS Code Reviewer | `rules_config` | Path to lint/review rules |

## Alternatives Considered

1. **Constructor syntax in source** (`host Worker(dir: String) { ... }`) — rejected because these are middleware deployment config, not language semantics.
2. **Environment variables only** — works but is unstructured, hard to document per-host, and doesn't compose well with multiple hosts.
3. **Extending `args` array** — CLI args are positional and untyped; named params are clearer and the middleware can parse them structurally.
