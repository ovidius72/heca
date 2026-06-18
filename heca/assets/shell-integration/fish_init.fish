function __heca_osc
    printf '\e]%s\a' $argv[1]
end

function __heca_emit_cwd
    set -l host (hostname)
    if test -z "$host"
        set host localhost
    end
    set -l cwd (string escape --style=url -- "$PWD")
    __heca_osc "7;file://$host$cwd"
end

set -g __heca_have_preexec 0

function __heca_preexec --on-event fish_preexec
    set -g __heca_have_preexec 1
    __heca_osc '133;B'
    __heca_osc '133;C'
end

function __heca_prompt_event --on-event fish_prompt
    set -l exit_status $status
    if test $__heca_have_preexec -eq 1
        __heca_osc "133;D;$exit_status"
    end
    __heca_osc '133;A'
    __heca_emit_cwd
    set -g __heca_have_preexec 0
end
