# Security policy

libSACM parses XML from untrusted sources, so parser and resource-exhaustion issues
are in scope, as are any ways a document could be silently altered on load or save.

## Reporting a vulnerability

Please **do not** open a public issue. Report privately through GitHub's
[private vulnerability reporting](https://github.com/lasrod/libSACM/security/advisories/new).

Include the version or commit, a minimal input that reproduces the issue, and what
you observed. You will get an acknowledgement within a week and, once confirmed, a
fix timeline. Credit is given in the advisory unless you prefer otherwise.

## Supported versions

Before 1.0, only the latest released minor version receives security fixes.
