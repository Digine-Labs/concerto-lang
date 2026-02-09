#!/usr/bin/env bash
set -euo pipefail

session=0

json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    printf '%s' "$value"
}

extract_response_value() {
    local line="$1"
    if [[ "$line" =~ \"value\":\"([^\"]*)\" ]]; then
        printf '%s' "${BASH_REMATCH[1]}"
    else
        printf ''
    fi
}

while IFS= read -r prompt_line; do
    if [[ -z "$prompt_line" ]]; then
        continue
    fi

    session=$((session + 1))
    escaped_prompt="$(json_escape "$prompt_line")"

    echo "{\"type\":\"progress\",\"message\":\"session ${session}: prompt received\",\"phase\":\"ingest\",\"turn\":${session}}"
    echo "{\"type\":\"question\",\"id\":\"session-${session}-q1\",\"question\":\"Choose an implementation plan for: ${escaped_prompt}\",\"kind\":\"design\",\"options\":[\"safe\",\"fast\"]}"

    if ! IFS= read -r answer_line; then
        exit 0
    fi
    decision="$(extract_response_value "$answer_line")"
    if [[ -z "$decision" ]]; then
        decision="no-decision"
    fi
    escaped_decision="$(json_escape "$decision")"

    echo "{\"type\":\"approval\",\"id\":\"session-${session}-a1\",\"description\":\"Apply decision '${escaped_decision}' to branch\",\"risk_level\":\"medium\"}"

    if ! IFS= read -r approval_line; then
        exit 0
    fi
    approval="$(extract_response_value "$approval_line")"
    if [[ -z "$approval" ]]; then
        approval="no"
    fi

    if [[ "$approval" == "yes" ]]; then
        echo "{\"type\":\"result\",\"text\":\"session ${session} complete\",\"session\":${session},\"prompt\":\"${escaped_prompt}\",\"decision\":\"${escaped_decision}\",\"approved\":\"${approval}\"}"
    else
        echo "{\"type\":\"error\",\"message\":\"session ${session} rejected by supervisor\"}"
    fi
done
