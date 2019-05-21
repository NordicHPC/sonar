
# Design goals

- pip installable
- minimal overhead for recording
- super quick reporting and dashboard, both stdout and csv for web postprocessing
- can be used as health check tool
- data filtering/mapping is asynchronous


# Design decisions

## `ps` instead of `top`

We started using `top` but it turned out that `top` is dependent on
locale, so it displays floats with comma instead of decimal point in
many non-English locales. `ps` always uses decimal points. In addition,
`ps` is (arguably) more versatile/configurable and does not print the
header that `top` prints. All these properties make the `ps` output
easier to parse than the `top` output.
