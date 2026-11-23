# heca shell integration: the hooks that tell heca where the shell is (OSC 7), when a command starts
# and ends with its status (OSC 133), and which program runs. Loaded by heca's .zshenv for an
# interactive shell; they attach at the first prompt, after the user's own configuration has run.

heca__osc() {
  print -rn -- $'\e]'"$1"$'\a'
}

heca__urlencode() {
  emulate -L zsh
  local input="$1"
  local output=""
  local i ch hex
  for ((i = 1; i <= ${#input}; i++)); do
    ch="$input[i]"
    case "$ch" in
      [[:alnum:].~_/-]) output+="$ch" ;;
      *)
        printf -v hex '%%%02X' "'$ch"
        output+="$hex"
        ;;
    esac
  done
  print -rn -- "$output"
}

heca__emit_cwd() {
  local host="${HOSTNAME:-localhost}"
  heca__osc "7;file://$host$(heca__urlencode "$PWD")"
}

heca__command_name() {
  emulate -L zsh
  local -a words
  words=(${(z)1})
  local cmd="${words[1]:-}"
  cmd="${cmd##*/}"
  print -rn -- "$cmd"
}

typeset -g HECA_HAVE_PREEXEC=0

heca_precmd() {
  local exit_status=$?
  if (( HECA_HAVE_PREEXEC )); then
    heca__osc "133;D;$exit_status"
  fi
  heca__osc "133;A"
  heca__emit_cwd
  HECA_HAVE_PREEXEC=0
  # **Forget where commands were found.** heca can turn an agent on after this shell started — its
  # launcher appears in $HECA_AGENT_BIN — and zsh would keep running the program it found first.
  [[ -n "${HECA_AGENT_BIN:-}" ]] && hash -r
  return "$exit_status"
}

heca_preexec() {
  HECA_HAVE_PREEXEC=1
  heca__osc "133;B"
  heca__osc "133;C"
  local prog
  prog="$(heca__command_name "$1")"
  [[ -n "$prog" ]] && heca__osc "133;E;$(heca__urlencode "$prog")"
  heca__osc "133;F;$(heca__urlencode "$1")"
}

# **Attach at the first prompt**, not now: the user's .zshrc runs after this file and may reset the
# hook arrays (some frameworks do). The first precmd runs after all of it, and removes itself.
heca__attach() {
  precmd_functions=(${precmd_functions:#heca__attach})
  # Programs heca bundles for agents come first on PATH — after the user's own PATH changes.
  if [[ -n "${HECA_AGENT_BIN:-}" ]]; then
    export PATH="$HECA_AGENT_BIN:$PATH"
  fi
  precmd_functions+=(heca_precmd)
  preexec_functions+=(heca_preexec)
  heca_precmd
}

typeset -ga precmd_functions
typeset -ga preexec_functions
precmd_functions+=(heca__attach)
