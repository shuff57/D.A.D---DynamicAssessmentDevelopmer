# Signing the desktop app, so school computers will run it

D.A.D ships as a portable app (`tools/package.sh`, built by `.github/workflows/desktop.yml`): no
installer, no admin rights. Whether a school computer runs it depends on signing and on the
district's own rules.

## What happens today (unsigned)

| | What a teacher sees | Works around it |
|---|---|---|
| **macOS** | "DAD can't be opened because Apple cannot check it for malicious software" (Gatekeeper), because the download is quarantined and the app is only ad-hoc signed. | Right-click DAD.app, Open, Open. A managed Mac may forbid this. |
| **Linux** | Nothing: runs after `tar xzf`. | — |
| **Windows** (later) | SmartScreen "Windows protected your PC". A district using AppLocker or WDAC blocks unsigned programs outright. | "More info, Run anyway", if allowed. |

## The plan

### macOS: Developer ID + notarization (do first)

1. Apple Developer Program, $99 a year (individual is fine; an organization account shows the
   school or business name).
2. A "Developer ID Application" certificate, kept as a GitHub Actions secret (base64 .p12 and its
   password), plus an App Store Connect API key for notarization.
3. In the workflow, replace the ad-hoc `codesign -s -` with
   `codesign --force --options runtime --timestamp -s "Developer ID Application: …" DAD.app`
   (hardened runtime; the app needs no extra entitlements: it starts Chrome and a local PHP as
   ordinary child processes and opens no listening ports beyond 127.0.0.1), then
   `xcrun notarytool submit DAD.zip --wait` and `xcrun stapler staple DAD.app`.
4. Result: double-click opens with no warning, even offline (stapled ticket).
5. The PHP downloaded on first use is not inside the signed app. It is checked against a pinned
   SHA-256 and written to the user's data folder, without the quarantine flag (the app writes it,
   not a browser), so Gatekeeper does not block it. If a district blocks unsigned executables
   entirely, ship PHP inside the app instead (`Contents/Resources/php`, signed with the app).

### Windows: Azure Trusted Signing (when Windows is added)

- Azure Trusted Signing: about $10 a month; signs with a Microsoft-issued certificate in CI
  (no hardware token). Needs identity validation (individual or organization).
- Alternative: an OV code-signing certificate (about $200–400 a year, on a hardware token or
  cloud HSM). EV certificates no longer give instant SmartScreen reputation; reputation builds
  with downloads either way.
- Sign `DAD.exe` and the downloaded `php.exe` path the same way as macOS (or bundle PHP and sign it).

### Linux

No signing system blocks apps. Publish SHA-256 sums beside the release files; optionally sign
them with a GPG key.

## Locked-down districts

Signing removes the warnings, but a district that allowlists software (AppLocker, WDAC, Jamf,
Intune) still decides. What helps their IT approve it:

- One publisher identity (the certificate) they can allowlist, instead of a file hash that changes
  every release.
- The portable layout: everything in one folder, data in the user's profile (or `DAD-data/` in
  portable mode), no services, no admin, no system changes.
- A short IT note: what it connects to (myopenmath.com through the teacher's own Chrome or Edge;
  the AI provider the teacher picks; dl.static-php.dev once for PHP), and that the checker listens
  on 127.0.0.1 only.

Where none of that is possible, the web app still works with no install: copy buttons, the
one-click bookmark, and the checker running inside the browser.
