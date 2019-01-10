

Design decisions
================

We made few design decisions which were controversially discussed. To allow our
future selves or other developers to not go through the same struggle again,
they are shortly summarized.


ps instead of top
-----------------

We started using ``top`` but it turned out that ``top`` is dependent on locale,
so it displays floats with comma instead of decimal point in many non-English
locales. ``ps`` always uses decimal points. In addition, ``ps`` is (arguably)
more versatile/configurable and does not print the header that ``top`` prints.
All these properties make the ``ps`` output easier to parse than the ``top``
output.
