# Host Response Correlation IDs

## Problem

Current bidirectional host streaming responses from Concerto to host use:

```json
{"type":"response","in_reply_to":"question","value":"..."}
```

`in_reply_to` carries only the message type (for example `question` or `approval`), not a unique request ID.

For hosts that send multiple requests of the same type in one session, correlating a specific response to a specific request is ambiguous.

## Proposal

Add optional correlation IDs to the host protocol:

- Host -> Concerto request messages include `id`.
- Concerto -> host response messages include `in_reply_to_id` (copied from request `id`).

Example:

```json
{"type":"question","id":"q-17","question":"Use RS256 or HS256?"}
{"type":"response","in_reply_to":"question","in_reply_to_id":"q-17","value":"Use RS256"}
```

## Migration Strategy

1. Keep current `in_reply_to` behavior for backward compatibility.
2. If request `id` exists, runtime adds `in_reply_to_id` to response.
3. Hosts can gradually adopt strict ID-based matching while still accepting legacy responses.

## Why This Matters

- Enables reliable request/response routing in concurrent or multi-step host workflows.
- Removes protocol ambiguity for repeated `question` / `approval` cycles.
- Improves robustness for real external agent middleware adapters.
