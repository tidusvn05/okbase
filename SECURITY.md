# Security Policy

Please report vulnerabilities **privately** through GitHub Security Advisories ("Report a vulnerability") on this repository. Do not open public issues for security problems.

Relevant areas include:
- path traversal outside a bundle;
- scope bypass (reading documents outside a host-provided `Scope`);
- SQL escaping the read-only `data_query` sandbox;
- MCP HTTP authentication;
- unsafe handling of imported files (PDF/DOCX/HTML).

We aim to acknowledge reports within 7 days. Supported versions: the latest 0.x release until 1.0.
