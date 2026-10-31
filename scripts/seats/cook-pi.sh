#!/bin/zsh
# Headless cook seat. Usage: cook-pi.sh <GO_NAME> <WORKTREE> <GO_FILE> [extra notes file]
# SEAT_COOK_MODEL unset: pi's default model, no --model flag.
# SEAT_COOK_MODEL set: OpenRouter only, and only when ZDR is confirmed. A route
# error logs STOP and exits 3. It does not rerun on pi-grok or any other model.
set -uo pipefail
GO="${1:-}"
WT="${2:-}"
GOFILE="${3:-}"
EXTRA="${4:-}"
G="${HOME}/Orbitty GOs"
mkdir -p "$G/_logs" "$G/_pi-cook-sessions"
LOG="$G/_logs/cook-pi_${GO}_$(date +%Y%m%d-%H%M%S)-${RANDOM}.log"
SEAT_DOC=${SEAT_DOC:-"$G/COOK_PI_SEAT.md"}
PROMPT="COOK sealed GO ${GO} in worktree ${WT}. GO file: ${GOFILE}. Implement it per your standing instructions, write the cook note, print the final status line."
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

route_line() {
  grep -E -i '401|402|404|model not found|no zdr provider' "$1" | head -n 1 || true
}

short_reason() {
  local line=$1
  case $line in
    *402*) print -r -- 402 ;;
    *401*) print -r -- 401 ;;
    *404*) print -r -- 404 ;;
    *[Mm]odel[[:space:]]not[[:space:]]found*) print -r -- "model not found" ;;
    *) print -r -- "no ZDR provider" ;;
  esac
}

print -r -- "cook-pi start $(date) go=$GO" > "$LOG"

if [[ -z $GO || -z $WT || -z $GOFILE ]]; then
  print -r -- "BLOCKED: ${GO:-cook} cook-pi missing args" >> "$LOG"
  finish_log 3
fi

[[ ! -d "$WT/src-tauri" ]] || "$HOME/bin/hangar-target.sh" "$WT" || { print -r -- "RIG BLOCKER: MiniHangar not ready for $WT (hangar-target.sh)" >> "$LOG"; finish_log 3; }

MODEL_ARGS=()
if [[ -n ${SEAT_COOK_MODEL:-} ]]; then
  if ! zdr_ok; then
    print -r -- "BLOCKED: $GO OpenRouter ZDR not confirmed" >> "$LOG"
    finish_log 3
  fi
  MODEL_ARGS=(--model "openrouter/${SEAT_COOK_MODEL}")
  print -r -- 'provider_prefs: {"zdr":true,"data_collection":"deny"}' >> "$LOG"
fi

cd "$WT" || { print -r -- "BLOCKED: $GO worktree missing" >> "$LOG"; finish_log 3; }
pi -p "${MODEL_ARGS[@]}" --append-system-prompt "$SEAT_DOC" --session-dir "$G/_pi-cook-sessions" --name "cook-${GO}" "$PROMPT" < /dev/null >> "$LOG" 2>&1
rc=$?

hit=$(route_line "$LOG")
if [[ -n $hit ]]; then
  print -r -- "STOP $(short_reason "$hit")" >> "$LOG"
  finish_log 3
fi
if grep -E -q '^(RIG BLOCKER|BLOCKED):' "$LOG"; then
  finish_log 3
fi
finish_log "$rc"
