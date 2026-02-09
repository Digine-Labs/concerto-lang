# bidirectional_host_middleware

Self-contained Spec 27 example for bidirectional host communication.

This project includes both sides of the protocol:

- `src/main.conc`: Concerto supervisor that uses `listen` handlers (`progress`, `question`, `approval`)
- `host/mock_external_agent.sh`: local host middleware that simulates an external agent system over NDJSON stdio

Run from repository root:

```bash
cargo run -p concertoc -- examples/bidirectional_host_middleware/src/main.conc
cargo run -p concerto -- run examples/bidirectional_host_middleware/src/main.conc-ir
```

Expected emit channels include:

- `listen:start`
- `host:progress`
- `host:question`
- `host:approval`
- `host:first_result`
- `host:second_result`
- `listen:complete`
