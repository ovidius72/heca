# heca shell integration: a login shell reads the user's .zprofile (Homebrew's PATH, for one).
if [[ -r "${OLD_ZDOTDIR:-$HOME}/.zprofile" ]]; then
  source "${OLD_ZDOTDIR:-$HOME}/.zprofile"
fi
