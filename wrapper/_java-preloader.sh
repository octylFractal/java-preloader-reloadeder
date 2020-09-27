#!/usr/bin/env bash

if ! command -v jpre 2>/dev/null >&2; then
    printf '%s\n' 'jpre is not installed. Get it via `cargo install jpre`.'
    return
fi

jpre() {
    if [[ $1 == use ]]; then
        # This is the only override we need to perform
        local code
        code="$(command jpre --shell-integration "$@")"
        exit_code=$?
        if [[ $exit_code -ne 0 ]]; then
            return $exit_code
        fi
        eval "$code" || { printf '%s\n' "Failed to evaluate jpre-use code!"; return 1; }
        printf '%s\n' "$(tput setaf 2)Now using $(command jpre current).$(tput sgr0)"
        return 0
    fi

    # Delegate to actual command.
    command jpre "$@"
    return $?
}
