#!/usr/bin/env bash

if ! command -v jpre 2>/dev/null >&2; then
    printf '%s\n' 'jpre is not installed. Get it via `cargo install jpre`.'
    return
fi

jpre() {
    if [[ $1 == use ]]; then
        # This is the only override we need to perform
        local code
        code="$(command jpre "$@")"
        exit_code=$?
        if [[ $exit_code -ne 0 ]]; then
            return $exit_code
        fi
        eval "$code" || { printf '%s\n' "Failed to evaluate jpre-use code!"; return 1; }
        printf '%s\n' "Now using $(command jpre current)."
        return 0
    fi

    # Delegate to actual command.
    command jpre "$@"
    return $?
}
