#!/bin/zsh
# Headless Verify seat. Usage: verify-pi.sh <GO_NAME> <TIP_SHA> <WORKTREE> <GO_FILE> [extra notes file]
# SEAT_VERIFY_MODEL unset: pi's default model, no --model flag.
# Set: one GLM call. If that call fails (nonzero exit, a timeout line, or a
# route string), one Sol-flex call, with verify_model_used and fallback_reason
# in the log. If Sol fails too, STOP exit 3. Never a third call, and never
# pi-grok.
set -uo pipefail
GO="${1:-}"
TIP="${2:-}"
WT="${3:-}"
GOFILE="${4:-}"
EXTRA="${5:-}"
S8="${TIP:0:8}"
G="${HOME}/Orbitty GOs"
mkdir -p "$G/_logs" "$G/_pi-verify-sessions"
LOG="$G/_logs/verify-pi_${GO}_${S8}_$(date +%Y%m%d-%H%M%S)-${RANDOM}.log"
SEAT_DOC=${SEAT_DOC:-"$G/VERIFY_PI_SEAT.md"}
PROMPT="VERIFY GO ${GO} at tip ${TIP} in worktree ${WT}. GO file: ${GOFILE}. Run its VERIFY checklist per your standing instructions, write the report, print the RESULT line."
[[ -n "$EXTRA" ]] && PROMPT="$PROMPT Extra context from Chuck: $EXTRA (read it)."
unset CARGO_TARGET_DIR

finish_log() {
  local rc=$1 tip=none
  if [[ -n ${WT:-} && -d ${WT:-} ]]; then
    tip=$(git -C "$WT" rev-parse HEAD 2>/dev/null || print -r -- none)
  fi
  print -r -- "EXIT $rc tip=$tip" >> "$LOG"
  tail -n 2 "$LOG"
  exit "$rc"
}

zdr_ok() {
  if [[ ${SEAT_PI_SENDS_PROVIDER_PREFS:-0} == 1 ]]; then
    return 0
  fi
  [[ -f "$HOME/.config/orbitty/openrouter-zdr-confirmed" ]]
}

# Lines from `from` (1-based) onward. Empty if this slice has no route error
# and no error-shaped timeout (the probe line is "error: request timeout").
# A bare mention of the word timeout is not a failure; a nonzero exit is.
route_line_from() {
  local log=$1 from=$2
  awk -v from="$from" 'NR >= from && /401|402|404|[Mm]odel not found|[Nn]o ZDR provider|[Rr]equest timeout|[Tt]imed out|error:.*[Tt]imeout/ { print; exit }' "$log" || true
}

short_reason() {
  local line=$1
  case $line in
    *402*) print -r -- 402 ;;
    *401*) print -r -- 401 ;;
    *404*) print -r -- 404 ;;
    *[Mm]odel[[:space:]]not[[:space:]]found*) print -r -- "model not found" ;;
    *[Tt]imeout*|*[Tt]imed[[:space:]]out*) print -r -- timeout ;;
    *) print -r -- "no ZDR provider" ;;
  esac
}

# Prefer the route/timeout line. A bare nonzero exit still has a reason.
fail_reason() {
  local line=$1 rc=$2
  if [[ -n $line ]]; then
    short_reason "$line"
    return
  fi
  print -r -- "exit $rc"
}

call_pi() {
  local model=$1
  local -a args=()
  if [[ -n $model ]]; then
    args=(--model "openrouter/${model}")
  fi
  pi -p "${args[@]}" --no-mcp --append-system-prompt "$SEAT_DOC" \
    --session-dir "$G/_pi-verify-sessions" --name "verify-${GO}-${S8}" \
    "$PROMPT" < /dev/null >> "$LOG" 2>&1
  return $?
}

print -r -- "verify-pi start $(date) go=$GO tip=$TIP" > "$LOG"

if [[ -z $GO || -z $TIP || -z $WT || -z $GOFILE ]]; then
  print -r -- "BLOCKED: ${GO:-verify} verify-pi missing args" >> "$LOG"
  finish_log 3
fi

[[ ! -d "$WT/src-tauri" ]] || "$HOME/bin/hangar-target.sh" "$WT" || { print -r -- "RIG BLOCKER: MiniHangar not ready for $WT (hangar-target.sh)" >> "$LOG"; finish_log 3; }

if [[ -n ${SEAT_VERIFY_MODEL:-} ]]; then
  if ! zdr_ok; then
    print -r -- "BLOCKED: $GO OpenRouter ZDR not confirmed" >> "$LOG"
    finish_log 3
  fi
  # pi cannot attach this object per request (S0). The marker is the live gate.
  print -r -- 'provider_prefs: {"zdr":true,"data_collection":"deny"} order: Novita,DeepInfra,Z.AI allow_fallbacks: false quantizations: fp8' >> "$LOG"
fi

cd "$WT" || { print -r -- "BLOCKED: $GO worktree missing" >> "$LOG"; finish_log 3; }

rc=0
if [[ -n ${SEAT_VERIFY_MODEL:-} ]]; then
  print -r -- "verify_model_used: ${SEAT_VERIFY_MODEL}" >> "$LOG"
  mark=$(wc -l < "$LOG" | tr -d '[:space:]')
  call_pi "$SEAT_VERIFY_MODEL" || rc=$?
  hit=$(route_line_from "$LOG" $((mark + 1)))
  if [[ $rc -ne 0 || -n $hit ]]; then
    backup=${SEAT_VERIFY_BACKUP:-openai/gpt-6.1-sol}
    print -r -- "fallback_reason: $(fail_reason "$hit" "$rc")" >> "$LOG"
    print -r -- "verify_model_used: $backup" >> "$LOG"
    mark2=$(wc -l < "$LOG" | tr -d '[:space:]')
    rc=0
    call_pi "$backup" || rc=$?
    hit2=$(route_line_from "$LOG" $((mark2 + 1)))
    if [[ $rc -ne 0 || -n $hit2 ]]; then
      print -r -- "STOP $(fail_reason "$hit2" "$rc")" >> "$LOG"
      finish_log 3
    fi
  fi
else
  call_pi "" || rc=$?
fi

if grep -E -q '^(RIG BLOCKER|BLOCKED):' "$LOG"; then
  finish_log 3
fi
finish_log "$rc"
