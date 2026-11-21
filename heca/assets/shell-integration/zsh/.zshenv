# heca shell integration: zsh read this file because heca pointed ZDOTDIR at this folder.
#
# Point ZDOTDIR back at the user's own folder at once, before anything else reads it. Every later
# startup file then behaves exactly as in any other terminal: macOS's /etc/zshrc picks the history
# file (HISTFILE=${ZDOTDIR:-$HOME}/.zsh_history), compinit its completion cache, plugin managers their
# folders — all from the user's folder, not heca's — and zsh reads the user's own .zprofile, .zshrc
# and .zlogin itself. heca's hooks are loaded from here and attach at the first prompt
# (heca-integration.zsh), after the user's configuration, so it cannot reset them.
_heca_dir="$ZDOTDIR"
# OLD_ZDOTDIR is the user's ZDOTDIR as heca found it (empty when there was none, which is $HOME).
if [[ -n "${OLD_ZDOTDIR-}" && "$OLD_ZDOTDIR" != "$_heca_dir" ]]; then
  export ZDOTDIR="$OLD_ZDOTDIR"
else
  unset ZDOTDIR
fi
unset OLD_ZDOTDIR
if [[ -r "${ZDOTDIR:-$HOME}/.zshenv" ]]; then
  source "${ZDOTDIR:-$HOME}/.zshenv"
fi
if [[ -o interactive && -r "$_heca_dir/heca-integration.zsh" ]]; then
  source "$_heca_dir/heca-integration.zsh"
fi
unset _heca_dir
