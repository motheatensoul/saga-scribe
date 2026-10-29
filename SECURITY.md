# Security Policy

Saga-Scribe is a local desktop application (Tauri: Rust backend, web frontend).
It has no server component and no user accounts. Its network use is limited to
ONP dictionary lookups. Security issues mostly concern **untrusted input
files** handled on the user's machine.

## Supported versions

Only the latest release receives security fixes.

| Version | Supported |
| ------- | --------- |
| 0.6.x   | Yes       |
| < 0.6   | No        |

## Reporting a vulnerability

**Do not open a public issue, pull request or discussion for a security problem.**

Report it privately through GitHub's private vulnerability reporting:
https://github.com/motheatensoul/saga-scribe/security/advisories/new

Please include:

- the affected version and platform (Linux .deb/AppImage/Flatpak, Windows, macOS);
- a minimal input that reproduces the problem (an XML file, XSLT stylesheet or `.teis` project), if possible;
- what happens, and what an attacker gains.

You can expect an acknowledgement within a week. This is a small volunteer
project, so fixes are released as time allows, with priority by severity.
Reporters are credited in the advisory unless they prefer not to be.

## Scope

In scope: anything where opening or importing a file, applying a stylesheet,
or exporting a document leads to code execution, file access outside what the
user chose, data loss, or network access the user didn't ask for. In particular:

- **Imported TEI-XML and plain text**: parsed with libxml2 without network
  access (no external DTDs or entities are fetched).
- **XSLT stylesheets**: their output is sanitized (DOMPurify) before it is
  displayed, exported or printed, and the app runs under a strict Content
  Security Policy.
- **Project files (`.teis`, zip archives)**: contents are read, never executed.
- **Exports (HTML, PDF, TEI-XML)**: written atomically to the path the user chose.

Out of scope:

- issues requiring an already compromised machine or user account;
- vulnerabilities in the operating system, WebKitGTK/WebView2, or the Flatpak
  runtime (report those upstream; we update when fixes are available);
- dependency advisories already reported by our CI audits, unless you can show
  they are exploitable in Saga-Scribe.

## For automated agents and contributors

- Never disclose a suspected vulnerability in public issues, pull request
  descriptions, commit messages or code comments before a fix is released.
  Report it privately as above, or tell the maintainer directly.
- Keep these protections intact when changing code:
  - render or export XSLT output only through `sanitizeXsltHtml()`
    (`src/lib/utils/xslt.ts`);
  - parse documents only through `crate::xml::parse` (`src-tauri/src/xml.rs`),
    which disables network access;
  - don't loosen the Content Security Policy in `src-tauri/tauri.conf.json`
    without a stated reason;
  - write files atomically (temp file + rename), as `src-tauri/src/commands/file.rs` does.
- Dependency audits (`cargo audit`, `bun audit`) run in CI; fix or upgrade
  rather than ignore findings, and record exceptions with a reason.
