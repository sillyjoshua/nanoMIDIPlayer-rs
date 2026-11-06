# Security

## Reporting a suspected issue

Report suspected vulnerabilities through GitHub Security Advisories or the repository issue tracker. Do not publish credentials, private data, or potentially harmful samples in a public issue.

## Windows behavior and Defender detections

This application has legitimate reasons to use Windows input APIs:

- The configurable global hotkeys currently use a low-level keyboard hook so the app can detect configured shortcuts and let users capture a shortcut while another application has focus.
- MIDI-to-keyboard playback uses `SendInput` to send the notes selected by the user.
- The optional pause-on-focus-loss feature checks the foreground window handle and process ID. It does not read window titles or collect the text typed into other applications.

The low-level hook is a known heuristic false-positive risk because the same Windows API is also used by keyloggers. The callback ignores injected key events and only passes key-down virtual-key codes to the app's hotkey handler. The app does not save arbitrary keystrokes or transmit them.

These behaviours are documented rather than hidden or obfuscated. Removing the hook entirely would break global hotkeys and the existing key-capture feature; a future replacement must preserve those features without collecting unrelated input.

## Updates

The application checks GitHub release metadata for this repository. It does not download, execute, or replace its own executable. Users open the official GitHub release page and choose when to download and install an update. The legacy `autoUpdate` configuration value is ignored; automatic installation is intentionally disabled.

## Telemetry

Usage telemetry is disabled by default. When explicitly enabled in settings, the app sends an anonymous app-running ping to `api.nanomidi.net`. No keystrokes, MIDI content, filenames, or personal documents are included in that ping.

## Dependency and build checks

The security workflow runs formatting, tests, Clippy, and `cargo audit` against the committed lockfile. Release artifacts are built from source by GitHub Actions.
