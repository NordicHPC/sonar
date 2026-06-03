#!/usr/bin/env bash

if [[ $(whoami) != "root" ]]; then
    echo "Run test as root"
    exit 1
fi

#REQ_EXE_FOR_PIDS=
./sonar-daemon-runner ./sonar-mock myconfig.cfg larstha larstha
