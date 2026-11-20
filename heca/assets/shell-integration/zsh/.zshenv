# heca shell integration: zsh reads every startup file from ZDOTDIR, which heca points here, so
# each file of heca's reads the user's own file of the same name first.
_heca_dir="$ZDOTDIR"
_heca_user="${OLD_ZDOTDIR:-$HOME}"
# Never this folder itself — reading it would start over from here, for this file and the later ones.
if [[ "$_heca_user" == "$_heca_dir" ]]; then
  _heca_user="$HOME"
  unset OLD_ZDOTDIR
fi
if [[ -r "$_heca_user/.zshenv" ]]; then
  ZDOTDIR="$_heca_user"
  source "$_heca_user/.zshenv"
  # The user's .zshenv may move its own config folder; the later files are read from there.
  OLD_ZDOTDIR="$ZDOTDIR"
fi
ZDOTDIR="$_heca_dir"
unset _heca_dir _heca_user
