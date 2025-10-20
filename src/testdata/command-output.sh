#!/usr/bin/env bash
echo "output from stdout"
echo "output from stderr" >/dev/stderr
if [[ -n $1 ]]; then
    exit $1
fi
