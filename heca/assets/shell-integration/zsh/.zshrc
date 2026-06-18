if [[ -r "${OLD_ZDOTDIR:-$HOME}/.zshrc" ]]; then
  source "${OLD_ZDOTDIR:-$HOME}/.zshrc"
fi

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

typeset -g HECA_HAVE_PREEXEC=0

heca_precmd() {
  local exit_status=$?
  if (( HECA_HAVE_PREEXEC )); then
    heca__osc "133;D;$exit_status"
  fi
  heca__osc "133;A"
  heca__emit_cwd
  HECA_HAVE_PREEXEC=0
  return "$exit_status"
}

heca_preexec() {
  HECA_HAVE_PREEXEC=1
  heca__osc "133;B"
  heca__osc "133;C"
}

autoload -Uz add-zsh-hook
add-zsh-hook precmd heca_precmd
add-zsh-hook preexec heca_preexec
