# Remove only Heca's wrapper directory from the command's current PATH.
# Keep empty entries: they mean the current directory in a shell search path.
heca_agent_remaining=$PATH
heca_agent_path=
heca_agent_separator=
while :; do
    case "$heca_agent_remaining" in
        *:*)
            heca_agent_entry=${heca_agent_remaining%%:*}
            heca_agent_remaining=${heca_agent_remaining#*:}
            heca_agent_more=1
            ;;
        *)
            heca_agent_entry=$heca_agent_remaining
            heca_agent_more=
            ;;
    esac
    if [ "$heca_agent_entry" != "$heca_agent_bin" ]; then
        heca_agent_path=$heca_agent_path$heca_agent_separator$heca_agent_entry
        heca_agent_separator=:
    fi
    [ -n "$heca_agent_more" ] || break
done
PATH=$heca_agent_path
export PATH
