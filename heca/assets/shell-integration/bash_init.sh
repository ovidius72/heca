# A login bash ignores --init-file, so heca starts an ordinary one and, where the user's shell
# should be a login shell (HECA_LOGIN_SHELL, macOS), reads the login files here as bash would:
# /etc/profile, then the first of ~/.bash_profile, ~/.bash_login, ~/.profile — which usually reads
# ~/.bashrc itself (and ~/.bashrc when there is no login file).
if [ -n "${HECA_LOGIN_SHELL:-}" ]; then
  unset HECA_LOGIN_SHELL
  if [ -r /etc/profile ]; then
    . /etc/profile
  fi
  if [ -r "$HOME/.bash_profile" ]; then
    . "$HOME/.bash_profile"
  elif [ -r "$HOME/.bash_login" ]; then
    . "$HOME/.bash_login"
  elif [ -r "$HOME/.profile" ]; then
    . "$HOME/.profile"
  elif [ -r "$HOME/.bashrc" ]; then
    # No login file at all: the user's only configuration is .bashrc, so it is not skipped.
    . "$HOME/.bashrc"
  fi
elif [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi

if [ -n "${HECA_AGENT_BIN:-}" ]; then
  export PATH="$HECA_AGENT_BIN:$PATH"
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

heca__command_name() {
  local cmd="$1"
  cmd="${cmd#"${cmd%%[![:space:]]*}"}"
  cmd="${cmd%%[[:space:];|&]*}"
  cmd="${cmd##*/}"
  printf '%s' "$cmd"
}

heca__prompt_command() {
  local exit_status="${1:-0}"
  HECA_IN_PROMPT_COMMAND=1
  if [ "${HECA_HAVE_PREEXEC:-0}" -eq 1 ] 2>/dev/null; then
    heca__osc "133;D;$exit_status"
  fi
  heca__osc "133;A"
  heca__emit_cwd
  HECA_HAVE_PREEXEC=0
  HECA_LAST_COMMAND=""
  if [ -n "$HECA_OLD_PROMPT_COMMAND" ]; then
    eval "$HECA_OLD_PROMPT_COMMAND"
  fi
  HECA_IN_PROMPT_COMMAND=0
  return "$exit_status"
}

heca__preexec() {
  [ "${HECA_IN_PROMPT_COMMAND:-0}" -eq 1 ] 2>/dev/null && return
  local cmd="${BASH_COMMAND:-}"
  case "$cmd" in heca__prompt_command*) return ;; esac
  local caller
  for caller in "${FUNCNAME[@]:1}"; do
    [ "$caller" = heca__prompt_command ] && return
  done
  [ -z "$cmd" ] && return
  [ "$cmd" = "${HECA_LAST_COMMAND:-}" ] && return
  HECA_LAST_COMMAND="$cmd"
  HECA_HAVE_PREEXEC=1
  heca__osc "133;B"
  heca__osc "133;C"
  local prog
  prog="$(heca__command_name "$cmd")"
  [ -n "$prog" ] && heca__osc "133;E;$(heca__urlencode "$prog")"
  heca__osc "133;F;$(heca__urlencode "$cmd")"
}

HECA_OLD_PROMPT_COMMAND="${PROMPT_COMMAND:-}"
HECA_IN_PROMPT_COMMAND=1
PROMPT_COMMAND='heca__prompt_command "$?"'
trap 'heca__preexec' DEBUG
