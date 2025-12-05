#----BEGIN INCLUDE common.sh
# NOTE: Do not directly edit this section, which is copied from "common.sh".
# To modify it, one can edit "common.sh" and run "./update.sh" to apply
# the changes. See code comments in "common.sh" for the implementation details.

__fzf_defaults() {
  printf '%s\n' "--height ${FZF_TMUX_HEIGHT:-40%} --min-height 20+ --bind=ctrl-z:ignore $1"
  command cat "${FZF_DEFAULT_OPTS_FILE-}" 2> /dev/null
  printf '%s\n' "${FZF_DEFAULT_OPTS-} $2"
}

__fzf_exec_awk() {
  if [[ -z ${__fzf_awk-} ]]; then
    __fzf_awk=awk
    if [[ $OSTYPE == solaris* && -x /usr/xpg4/bin/awk ]]; then
      __fzf_awk=/usr/xpg4/bin/awk
    elif command -v mawk > /dev/null 2>&1; then
      local n x y z d
      IFS=' .' read -r n x y z d <<< $(command mawk -W version 2> /dev/null)
      [[ $n == mawk ]] &&
        (((x * 1000 + y) * 1000 + z >= 1003004)) 2> /dev/null &&
        ((d >= 20230302)) 2> /dev/null &&
        __fzf_awk=mawk
    fi
  fi
  LC_ALL=C exec "$__fzf_awk" "$@"
}
#----END INCLUDE

__fzf_comprun() {
  if [[ "$(type -t _fzf_comprun 2>&1)" == function ]]; then
    _fzf_comprun "$@"
  elif [[ -n ${TMUX_PANE-} ]] && { [[ ${FZF_TMUX:-0} != 0 ]] || [[ -n ${FZF_TMUX_OPTS-} ]]; }; then
    shift
    fzf-tmux ${FZF_TMUX_OPTS:--d${FZF_TMUX_HEIGHT:-40%}} -- "$@"
  else
    shift
    fzf "$@"
  fi
}

__fzf_orig_completion() {
  local l comp f cmd
  while read -r l; do
    if [[ $l =~ ^(.*\ -F)\ *([^ ]*).*\ ([^ ]*)$ ]]; then
      comp="${BASH_REMATCH[1]}"
      f="${BASH_REMATCH[2]}"
      cmd="${BASH_REMATCH[3]}"
      [[ $f == _fzf_* ]] && continue
      printf -v "_fzf_orig_completion_${cmd//[^A-Za-z0-9_]/_}" "%s" "${comp} %s ${cmd} #${f}"
      if [[ $l == *" -o nospace "* ]] && [[ ${__fzf_nospace_commands-} != *" $cmd "* ]]; then
        __fzf_nospace_commands="${__fzf_nospace_commands-} $cmd "
      fi
    fi
  done
}

# @param $1 cmd - Command name for which the original completion is searched
# @var[out] REPLY - Original function name is returned
__fzf_orig_completion_get_orig_func() {
  local cmd orig_var orig
  cmd=$1
  orig_var="_fzf_orig_completion_${cmd//[^A-Za-z0-9_]/_}"
  orig="${!orig_var-}"
  REPLY="${orig##*#}"
  [[ $REPLY ]] && type "$REPLY" &> /dev/null
}

# @param $1 cmd - Command name for which the original completion is searched
# @param $2 func - Fzf's completion function to replace the original function
# @var[out] REPLY - Completion setting is returned as a string to "eval"
__fzf_orig_completion_instantiate() {
  local cmd func orig_var orig
  cmd=$1
  func=$2
  orig_var="_fzf_orig_completion_${cmd//[^A-Za-z0-9_]/_}"
  orig="${!orig_var-}"
  orig="${orig%#*}"
  [[ $orig == *' %s '* ]] || return 1
  printf -v REPLY "$orig" "$func"
}

_fzf_opts_completion() {
  local cur prev opts
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  prev="${COMP_WORDS[COMP_CWORD - 1]}"
  opts="
    +c --no-color
    +i --no-ignore-case
    +s --no-sort
    +x --no-extended
    --ansi
    --bash
    --bind
    --border
    --border-label
    --border-label-pos
    --color
    --cycle
    --disabled
    --ellipsis
    --expect
    --filepath-word
    --fish
    --header
    --header-first
    --header-lines
    --height
    --highlight-line
    --history
    --history-size
    --hscroll-off
    --info
    --jump-labels
    --keep-right
    --layout
    --listen
    --listen-unsafe
    --literal
    --man
