if [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi

heca__osc() {
  printf '\033]%s\a' "$1"
}

heca__urlencode() {
  local input="$1"
  local output=""
  local i ch hex
  LC_ALL=C
  for ((i = 0; i < ${#input}; i++)); do
    ch="${input:i:1}"
    case "$ch" in
      [a-zA-Z0-9.~_/-]) output+="$ch" ;;
      *)
        printf -v hex '%%%02X' "'$ch"
        output+="$hex"
        ;;
    esac
  done
  printf '%s' "$output"
}

heca__emit_cwd() {
  local host="${HOSTNAME:-localhost}"
  heca__osc "7;file://$host$(heca__urlencode "${PWD:-/}")"
}

heca__prompt_command() {
  local exit_status="${1:-0}"
  if [ "${HECA_HAVE_PREEXEC:-0}" -eq 1 ] 2>/dev/null; then
    heca__osc "133;D;$exit_status"
  fi
  heca__osc "133;A"
  heca__emit_cwd
  HECA_HAVE_PREEXEC=0
  HECA_LAST_COMMAND=""
  return "$exit_status"
}

heca__preexec() {
  [ "${HECA_IN_PROMPT_COMMAND:-0}" -eq 1 ] 2>/dev/null && return
  local cmd="${BASH_COMMAND:-}"
  [ -z "$cmd" ] && return
  [ "$cmd" = "${HECA_LAST_COMMAND:-}" ] && return
  HECA_LAST_COMMAND="$cmd"
  HECA_HAVE_PREEXEC=1
  heca__osc "133;B"
  heca__osc "133;C"
}

HECA_OLD_PROMPT_COMMAND="${PROMPT_COMMAND:-}"
trap 'heca__preexec' DEBUG
if [ -n "$HECA_OLD_PROMPT_COMMAND" ]; then
  PROMPT_COMMAND='__heca_status=$?; HECA_IN_PROMPT_COMMAND=1; heca__prompt_command "$__heca_status"; HECA_IN_PROMPT_COMMAND=0; eval "$HECA_OLD_PROMPT_COMMAND"'
else
  PROMPT_COMMAND='__heca_status=$?; HECA_IN_PROMPT_COMMAND=1; heca__prompt_command "$__heca_status"; HECA_IN_PROMPT_COMMAND=0'
fi
