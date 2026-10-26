#!/usr/bin/env bash
#
# This is an interactive test.  Run as yourself on a node with several users active where you can
# sudo.  You should be able to observe that the output has non-blank "exe" fields for several users.

( cd .. ; cargo build )
sudo ./sonar-daemon-runner ../target/debug/sonar test.cfg $(whoami) $(groups | awk '{ print $1 }')

