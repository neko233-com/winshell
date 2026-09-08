# Loaded only by WinShell; your existing dotfiles remain untouched.
[[ -r /etc/profile ]] && source /etc/profile
if [[ -r ~/.bash_profile ]]; then
    source ~/.bash_profile
elif [[ -r ~/.bash_login ]]; then
    source ~/.bash_login
elif [[ -r ~/.profile ]]; then
    source ~/.profile
elif [[ -r ~/.bashrc ]]; then
    source ~/.bashrc
fi

[[ -z ${HISTCONTROL:-} ]] && HISTCONTROL=ignoreboth
shopt -s histappend checkwinsize
bind 'set completion-ignore-case on'
bind 'set show-all-if-ambiguous on'
bind 'set colored-stats on'
bind 'set colored-completion-prefix on'
bind 'set enable-bracketed-paste on'
bind '"\e[A": history-search-backward'
bind '"\e[B": history-search-forward'

__winshell_prompt() {
    local code=$?
    history -a
    local location
    location=$(cygpath -am "$PWD" 2>/dev/null) || location=$PWD
    # Control characters must not be allowed to escape the OSC payload.
    location=${location//$'\e'/}
    location=${location//$'\a'/}
    printf '\e]7;file://localhost/%s\a\e]133;A;%s\a' "$location" "$code"
}
# Preserve existing prompt callbacks and their ordering.
if declare -p PROMPT_COMMAND 2>/dev/null | grep -q 'declare -a'; then
    PROMPT_COMMAND=(__winshell_prompt "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND=(__winshell_prompt "${PROMPT_COMMAND:-:}")
fi
PS0=$'\e]133;C\a'
PS1='\[\e[38;2;116;213;187m\]\w\[\e[0m\]'
if declare -F __git_ps1 >/dev/null; then
    PS1+='\[\e[38;2;147;155;179m\]$(__git_ps1 "  %s")\[\e[0m\]'
fi
PS1+='\n\[\e[38;2;116;213;187m\]❯\[\e[0m\] \[\e]133;B\a\]'

