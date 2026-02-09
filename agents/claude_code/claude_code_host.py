#!/usr/bin/env python3
"""Concerto <-> Claude Code host middleware.

This process speaks the Concerto host protocol over stdio:
- stdin: prompt lines from Concerto runtime
- stdout: NDJSON messages with a `type` field

Modes:
- oneshot: emit a single `result` per prompt (safe for `Host.execute()`)
- stream: emit progress/partial/result messages (for `listen` workflows)

Interactive mode (stream only) optionally asks Concerto-side handlers for
clarifications (`question`) and approvals (`approval`) before running Claude.
"""

from __future__ import annotations

import argparse
import json
import os
import select
import shlex
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Any


def env_bool(name: str, default: bool) -> bool:
    value = os.getenv(name)
    if value is None:
        return default
    return value.strip().lower() in {"1", "true", "yes", "on"}


def env_int(name: str, default: int) -> int:
    value = os.getenv(name)
    if value is None:
        return default
    try:
        return int(value)
    except ValueError:
        return default


@dataclass
class HostOptions:
    mode: str
    interactive: bool
    mock: bool
    verbose: bool
    claude_command: str
    claude_args: list[str]
    prompt_mode: str
    timeout_secs: int
    response_timeout_secs: int
    history_turns: int
    max_partials: int
    carry_history: bool


class ClaudeCodeHost:
    def __init__(self, options: HostOptions) -> None:
        self.options = options
        self.session_counter = 0
        self.history: list[dict[str, str]] = []

    def run(self) -> None:
        self.log(
            "starting"
            f" mode={self.options.mode}"
            f" interactive={self.options.interactive}"
            f" mock={self.options.mock}"
        )

        for raw_line in sys.stdin:
            line = raw_line.strip()
            if not line:
                continue

            parsed = self.try_parse_json(line)
            if isinstance(parsed, dict) and parsed.get("type") == "response":
                self.log("ignored stray response message in main loop")
                continue

            prompt, context = self.parse_prompt_input(line, parsed)
            if not prompt:
                self.emit("error", message="received empty prompt")
                continue

            self.session_counter += 1
            session = self.session_counter
            started = time.time()

            try:
                self.process_prompt(session, prompt, context, started)
            except Exception as exc:  # pragma: no cover - defensive boundary
                self.emit("error", session=session, message=f"session failed: {exc}")

    def process_prompt(
        self,
        session: int,
        prompt: str,
        context: Any,
        started_at: float,
    ) -> None:
        if self.options.mode == "stream":
            self.emit(
                "progress",
                session=session,
                stage="received_prompt",
                message="Prompt received by claude_code middleware",
            )

        supervisor_note = ""
        if self.options.mode == "stream" and self.options.interactive:
            supervisor_note = self.request_question(
                question="Any extra implementation guidance before I run Claude Code?",
                context="Reply with one short instruction, or leave empty.",
                options=["Proceed as-is", "Prioritize safety", "Prioritize speed"],
            )
            if supervisor_note:
                self.emit(
                    "progress",
                    session=session,
                    stage="supervisor_guidance",
                    message="Applied supervisor guidance",
                )

        if (
            self.options.mode == "stream"
            and self.options.interactive
            and self.looks_high_risk(prompt)
        ):
            approved = self.request_approval(
                description=f"High-risk task detected: {self.summarize(prompt, 160)}",
                risk_level="high",
            )
            if not approved:
                self.emit(
                    "error",
                    session=session,
                    message="Task rejected by supervisor approval handler",
                )
                return

        composed_prompt = self.compose_prompt(prompt, context, supervisor_note)

        if self.options.mode == "stream":
            self.emit(
                "progress",
                session=session,
                stage="invoke_claude",
                message="Invoking Claude Code CLI",
            )

        raw_output, stderr_output, argv = self.invoke_claude(composed_prompt)
        result_text, partials, event_markers = self.parse_claude_output(raw_output)

        if self.options.mode == "stream":
            for marker in event_markers:
                self.emit(
                    "progress",
                    session=session,
                    stage="claude_event",
                    message=marker,
                )

            sent = 0
            for chunk in partials:
                if sent >= max(0, self.options.max_partials):
                    break
                if not chunk:
                    continue
                self.emit("partial", session=session, chunk=chunk)
                sent += 1

        elapsed_ms = int((time.time() - started_at) * 1000)
        self.emit(
            "result",
            session=session,
            text=result_text,
            elapsed_ms=elapsed_ms,
            command=argv[0] if argv else self.options.claude_command,
        )

        self.record_history(prompt, result_text)

        stderr_trimmed = (stderr_output or "").strip()
        if stderr_trimmed:
            self.log(f"session {session} stderr: {self.summarize(stderr_trimmed, 300)}")

    def invoke_claude(self, prompt: str) -> tuple[str, str, list[str]]:
        if self.options.mock:
            return self.mock_output(prompt), "", [self.options.claude_command, "<mock>"]

        argv, stdin_payload = self.build_command(prompt)
        self.log("exec: " + " ".join(shlex.quote(part) for part in argv))

        try:
            completed = subprocess.run(
                argv,
                input=stdin_payload,
                capture_output=True,
                text=True,
                timeout=max(1, self.options.timeout_secs),
                check=False,
            )
        except FileNotFoundError as exc:
            raise RuntimeError(
                f"Claude command not found: {self.options.claude_command}"
            ) from exc
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError(
                f"Claude command timed out after {self.options.timeout_secs}s"
            ) from exc

        if completed.returncode != 0:
            stderr_text = (completed.stderr or "").strip()
            stdout_text = (completed.stdout or "").strip()
            detail = stderr_text or stdout_text or f"exit code {completed.returncode}"
            raise RuntimeError(f"Claude command failed: {detail}")

        return completed.stdout or "", completed.stderr or "", argv

    def build_command(self, prompt: str) -> tuple[list[str], str | None]:
        args = list(self.options.claude_args)
        stdin_payload: str | None = None

        if any("{prompt}" in part for part in args):
            args = [part.replace("{prompt}", prompt) for part in args]
        elif self.options.prompt_mode == "stdin":
            stdin_payload = prompt + "\n"
        elif self.options.prompt_mode == "json-stdin":
            stdin_payload = json.dumps({"prompt": prompt}) + "\n"
        else:
            args.append(prompt)

        return [self.options.claude_command, *args], stdin_payload

    def parse_claude_output(self, output: str) -> tuple[str, list[str], list[str]]:
        lines = [line.strip() for line in output.splitlines() if line.strip()]
        partials: list[str] = []
        event_markers: list[str] = []

        for line in lines:
            parsed = self.try_parse_json(line)
            if isinstance(parsed, dict):
                extracted = self.extract_text(parsed)
                if extracted:
                    partials.append(extracted)
                else:
                    event_name = (
                        parsed.get("type")
                        or parsed.get("event")
                        or parsed.get("kind")
                        or "event"
                    )
                    event_markers.append(f"claude:{event_name}")
                continue

            partials.append(line)

        partials = self.compact_values(partials)
        event_markers = self.compact_values(event_markers)

        result_text = "\n".join(partials).strip()
        if not result_text:
            result_text = output.strip()
        if not result_text:
            result_text = "Claude Code returned no output."

        return result_text, partials, event_markers

    def compose_prompt(self, prompt: str, context: Any, supervisor_note: str) -> str:
        sections: list[str] = []

        if self.options.carry_history and self.history and self.options.history_turns > 0:
            sections.append("Recent middleware history:")
            for idx, item in enumerate(self.history[-self.options.history_turns :], start=1):
                sections.append(
                    f"{idx}. Prompt: {item['prompt']}\n"
                    f"   Result: {item['result']}"
                )

        sections.append(f"Task:\n{prompt}")

        if supervisor_note.strip():
            sections.append(f"Supervisor guidance:\n{supervisor_note.strip()}")

        if context is not None:
            if isinstance(context, str):
                context_text = context
            else:
                context_text = json.dumps(context, indent=2, sort_keys=True)
            sections.append(f"Concerto context:\n{context_text}")

        return "\n\n".join(sections)

    def record_history(self, prompt: str, result: str) -> None:
        if not self.options.carry_history:
            return

        self.history.append(
            {
                "prompt": self.summarize(prompt, 300),
                "result": self.summarize(result, 300),
            }
        )

        max_entries = max(1, self.options.history_turns * 2)
        if len(self.history) > max_entries:
            self.history = self.history[-max_entries:]

    def request_question(
        self,
        question: str,
        context: str,
        options: list[str],
    ) -> str:
        self.emit(
            "question",
            id=f"session-{self.session_counter}-question",
            question=question,
            context=context,
            options=options,
        )
        return self.read_response(default="")

    def request_approval(self, description: str, risk_level: str) -> bool:
        self.emit(
            "approval",
            id=f"session-{self.session_counter}-approval",
            description=description,
            risk_level=risk_level,
        )
        decision = self.read_response(default="no").strip().lower()
        return decision in {
            "y",
            "yes",
            "approve",
            "approved",
            "true",
            "1",
            "ok",
            "proceed",
        }

    def read_response(self, default: str) -> str:
        line = self.read_line_with_timeout(max(0, self.options.response_timeout_secs))
        if line is None:
            self.log("response timeout reached; using default")
            return default

        parsed = self.try_parse_json(line.strip())
        if isinstance(parsed, dict):
            value = parsed.get("value")
            if value is None:
                value = parsed.get("response")
            if value is None:
                return default
            if isinstance(value, str):
                return value.strip()
            return json.dumps(value, separators=(",", ":"))

        stripped = line.strip()
        return stripped if stripped else default

    @staticmethod
    def read_line_with_timeout(timeout_secs: int) -> str | None:
        if timeout_secs <= 0:
            line = sys.stdin.readline()
            return line if line else None

        try:
            ready, _, _ = select.select([sys.stdin], [], [], timeout_secs)
        except (ValueError, OSError):
            ready = [sys.stdin]

        if not ready:
            return None

        line = sys.stdin.readline()
        return line if line else None

    @staticmethod
    def parse_prompt_input(line: str, parsed: Any) -> tuple[str, Any]:
        if isinstance(parsed, dict) and "prompt" in parsed:
            prompt_value = parsed.get("prompt", "")
            if isinstance(prompt_value, str):
                prompt_text = prompt_value
            else:
                prompt_text = json.dumps(prompt_value, separators=(",", ":"))
            return prompt_text, parsed.get("context")
        return line, None

    @staticmethod
    def looks_high_risk(prompt: str) -> bool:
        text = prompt.lower()
        keywords = [
            "rm -rf",
            "drop table",
            "delete",
            "destroy",
            "production",
            "migrate",
            "rollback",
            "truncate",
            "sudo",
        ]
        return any(keyword in text for keyword in keywords)

    @staticmethod
    def extract_text(value: Any) -> str | None:
        if isinstance(value, str):
            text = value.strip()
            return text if text else None

        if isinstance(value, dict):
            for key in ("text", "completion", "delta", "output"):
                extracted = ClaudeCodeHost.extract_text(value.get(key))
                if extracted:
                    return extracted

            message = ClaudeCodeHost.extract_text(value.get("message"))
            if message:
                return message

            content = ClaudeCodeHost.extract_text(value.get("content"))
            if content:
                return content

            return None

        if isinstance(value, list):
            parts: list[str] = []
            for item in value:
                extracted = ClaudeCodeHost.extract_text(item)
                if extracted:
                    parts.append(extracted)
            if parts:
                return "\n".join(parts)

        return None

    @staticmethod
    def compact_values(values: list[str]) -> list[str]:
        compact: list[str] = []
        for value in values:
            if not value:
                continue
            if compact and compact[-1] == value:
                continue
            compact.append(value)
        return compact

    @staticmethod
    def try_parse_json(text: str) -> Any:
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return None

    @staticmethod
    def summarize(text: str, limit: int) -> str:
        text = text.replace("\n", " ").strip()
        if len(text) <= limit:
            return text
        return text[: max(0, limit - 3)] + "..."

    @staticmethod
    def mock_output(prompt: str) -> str:
        task = ClaudeCodeHost.summarize(prompt, 100)
        return "\n".join(
            [
                json.dumps(
                    {
                        "event": "analysis",
                        "text": f"Planning implementation for task: {task}",
                    }
                ),
                json.dumps(
                    {
                        "event": "edit",
                        "text": "Would update source files and add/adjust tests.",
                    }
                ),
                json.dumps(
                    {
                        "event": "final",
                        "text": "Mock Claude Code run completed successfully.",
                    }
                ),
            ]
        )

    def emit(self, msg_type: str, **payload: Any) -> None:
        message = {"type": msg_type}
        message.update(payload)
        sys.stdout.write(json.dumps(message, separators=(",", ":")) + "\n")
        sys.stdout.flush()

    def log(self, message: str) -> None:
        if self.options.verbose:
            sys.stderr.write(f"[claude_code_host] {message}\n")
            sys.stderr.flush()


def parse_args() -> HostOptions:
    parser = argparse.ArgumentParser(description="Concerto Claude Code host middleware")
    parser.add_argument(
        "--mode",
        choices=["oneshot", "stream"],
        default=os.getenv("CONCERTO_HOST_MODE", "oneshot"),
        help="oneshot emits one result line; stream emits progress/partial/result",
    )
    parser.add_argument(
        "--claude-command",
        default=os.getenv("CLAUDE_CODE_COMMAND", "claude"),
        help="command used to invoke Claude Code CLI",
    )
    parser.add_argument(
        "--claude-args",
        default=os.getenv("CLAUDE_CODE_ARGS", "--print"),
        help="argument string passed to the Claude command",
    )
    parser.add_argument(
        "--prompt-mode",
        choices=["arg-last", "stdin", "json-stdin"],
        default=os.getenv("CLAUDE_CODE_PROMPT_MODE", "arg-last"),
        help="how to pass prompt to Claude command",
    )
    parser.add_argument(
        "--timeout-secs",
        type=int,
        default=env_int("CLAUDE_CODE_TIMEOUT_SECS", 600),
        help="timeout for Claude command execution",
    )
    parser.add_argument(
        "--response-timeout-secs",
        type=int,
        default=env_int("CONCERTO_HOST_RESPONSE_TIMEOUT_SECS", 30),
        help="timeout waiting for supervisor responses in interactive mode",
    )
    parser.add_argument(
        "--history-turns",
        type=int,
        default=env_int("CONCERTO_HOST_HISTORY_TURNS", 3),
        help="number of recent turns to fold into new prompts",
    )
    parser.add_argument(
        "--max-partials",
        type=int,
        default=env_int("CONCERTO_HOST_MAX_PARTIALS", 32),
        help="max partial messages emitted per streamed session",
    )

    parser.add_argument("--interactive", dest="interactive", action="store_true")
    parser.add_argument("--no-interactive", dest="interactive", action="store_false")
    parser.set_defaults(interactive=env_bool("CONCERTO_HOST_INTERACTIVE", False))

    parser.add_argument("--mock", dest="mock", action="store_true")
    parser.add_argument("--no-mock", dest="mock", action="store_false")
    parser.set_defaults(mock=env_bool("CONCERTO_HOST_MOCK", False))

    parser.add_argument("--verbose", dest="verbose", action="store_true")
    parser.add_argument("--no-verbose", dest="verbose", action="store_false")
    parser.set_defaults(verbose=env_bool("CONCERTO_HOST_VERBOSE", False))

    parser.add_argument("--history", dest="carry_history", action="store_true")
    parser.add_argument("--no-history", dest="carry_history", action="store_false")
    parser.set_defaults(carry_history=env_bool("CONCERTO_HOST_CARRY_HISTORY", True))

    args = parser.parse_args()

    claude_args = shlex.split(args.claude_args) if args.claude_args else []

    return HostOptions(
        mode=args.mode,
        interactive=args.interactive,
        mock=args.mock,
        verbose=args.verbose,
        claude_command=args.claude_command,
        claude_args=claude_args,
        prompt_mode=args.prompt_mode,
        timeout_secs=args.timeout_secs,
        response_timeout_secs=args.response_timeout_secs,
        history_turns=max(0, args.history_turns),
        max_partials=max(0, args.max_partials),
        carry_history=args.carry_history,
    )


def main() -> int:
    options = parse_args()
    host = ClaudeCodeHost(options)
    host.run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
