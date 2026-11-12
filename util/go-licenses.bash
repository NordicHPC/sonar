#!/usr/bin/env bash
#
# Scan all Go modules and collect the licenses recursively and emit them to go-licenses.txt.  Then
# grep for infective licenses we should not have.

if [[ -z $SUBDIRS ]]; then
    echo ""
    echo "SUBDIRS is empty; see Makefile"
    echo ""
    exit 1
fi
if [[ ! -x $(command -v go-licenses) ]]; then
    echo ""
    echo "go-licenses not installed; see Makefile"
    echo ""
    exit 1
fi
trap 'rm -f go-licenses.txt.tmp' EXIT
rm -f go-licenses.txt.tmp
for d in $SUBDIRS; do
    # echo "Processing $d/"
    ( cd $d
      go mod download
      go-licenses report . ) 2>/dev/null >> go-licenses.txt.tmp
done
cat go-licenses.txt.tmp | awk -F, '{ print $1,"   ",$3 }' | sort | uniq > go-licenses.txt
if grep -i GPL go-licenses.txt; then
    echo ""
    echo '********************'
    echo "Infective license found, should not have that"
    exit 1
fi
