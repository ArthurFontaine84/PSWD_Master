# Security policy

PSWD Master is a personal project and has **not been independently audited**. It was designed with security as the first constraint, but please weigh that before trusting it with critical secrets.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's private reporting instead: *Security* tab → *Report a vulnerability*. Include the affected component (core, app, server), the version or commit, and steps to reproduce. I will acknowledge the report as soon as I can and keep you informed until it is fixed.

## Threat model

**Protected against**

- **Theft of a device's files, a backup or the server's database**: vaults are encrypted at rest (XChaCha20-Poly1305, key wrapped by Argon2id). Without the master password or the recovery key, a vault cannot be read.
- **A compromised or curious sync server** (reachable on the home network only for now; remote access is planned): the server stores opaque encrypted files and never receives a key or plaintext. Tampering is detected by authenticated encryption; a rejected remote copy never overwrites the local one.
- **Network attackers**: TLS 1.3 only, and the client pins the server certificate's SHA-256 fingerprint (no reliance on certificate authorities).
- **Malicious files** (crafted vault, backup or CSV): every file is parsed and validated in full before any key derivation, Argon2 parameters are bounded, parsers are fuzzed, and errors never panic.
- **Secret leakage through the app itself**: secrets are zeroed on drop and kept out of logs, `Debug` output and error messages; copied passwords are cleared from the clipboard after 20 s.

**Out of scope**

- A device that is already compromised (malware, keylogger, memory dump while a vault is unlocked).
- A weak master password chosen by the user (the app requires at least 12 characters and 3 character classes).
- Loss of both the master password and the recovery key: by design, nobody can recover the vault.

## Known limitations

These are deliberate trade-offs, documented in the specification ([`docs/SPEC.md`](../docs/SPEC.md), §9):

- **Web view input**: the master password and entry passwords are typed into HTML fields, so they briefly pass through JavaScript, which cannot zero memory. Fields are cleared as soon as they are submitted.
- **Clipboard on Linux**: exclusion from clipboard history depends on the clipboard manager (a warning is shown). A killed process cannot clear the clipboard.
- **Old copies after a password change**: changing the master password does not rotate the vault key, so an old backup plus the old password still opens that backup. Key rotation is planned as an extension point.
- **Clock skew**: merge conflicts are resolved by modification time; a device with a badly wrong clock can make the wrong version win (the other one stays in the entry's history).

## Supported versions

Only the latest release on the `main` branch receives fixes.
