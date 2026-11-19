# heca shell integration: a login shell reads the user's .zlogin.
if [[ -r "${OLD_ZDOTDIR:-$HOME}/.zlogin" ]]; then
  source "${OLD_ZDOTDIR:-$HOME}/.zlogin"
fi
