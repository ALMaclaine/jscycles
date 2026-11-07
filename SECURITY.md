# Security reporting and support

Security-response owner: repository maintainer alm.

The public GitHub API confirmed on 2026-11-06 that private vulnerability reporting
is **disabled** for the canonical repository `ALMaclaine/jscycles` (the former
`EnderHub/jscycles` URL redirects there). The
[advisory form](https://github.com/ALMaclaine/jscycles/security/advisories/new)
is therefore not an established reporting route. The read-only response is
retained in the [Linux/forge verification record](docs/rust-oe-linux-verification.json).
No other operational private route has been verified. Do not post exploitable
details, credentials or private source in public issues.

Supported security-update versions, a private route and an alternate recovery
owner still require an actual maintainer decision/verification. This is an OPEN
adherence gap, not a completed security policy. The unreleased 0.2 migration notes
do not promise that old versions receive backports.

The [maintainer runbook](docs/maintainer-runbook.md) defines triage, coordinated
disclosure, patch verification, release communication and incident closure once
a report is received. It makes no response deadline or external-standard claim.
