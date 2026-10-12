# MoodleDesk on Android and iOS

This branch (`mobile`) makes the same app run on phones and tablets. Tablets (600 px wide and up) keep the sidebar; phones get a bottom navigation bar.

## Build it

You need the usual Tauri mobile setup: <https://v2.tauri.app/start/prerequisites/> (Android Studio + NDK for Android, Xcode on a Mac for iOS).

```
npm install
npm run tauri android init      # once; creates src-tauri/gen/android
npm run tauri ios init          # once, on a Mac; creates src-tauri/gen/apple
npm run tauri android dev       # or: npm run tauri ios dev
npm run tauri android build     # or: npm run tauri ios build
```

Commit `src-tauri/gen/android` and `src-tauri/gen/apple` after the first `init`. For iOS set your team in `APPLE_DEVELOPMENT_TEAM` (or `bundle.iOS.developmentTeam`).

## What had to change, and why

| Problem on a phone | What the branch does |
| --- | --- |
| No app entry point | `run()` is a Tauri mobile entry point |
| Single-instance plugin is desktop-only | Only registered (and only compiled) on desktop |
| `keyring` has no Android backend | Android encrypts the login with a key that stays in the Android Keystore (secure hardware where the phone has it) and stores only the ciphertext, so backups of it are useless; iOS still uses the Keychain (`secret` in `lib.rs`, `plugins/android-native`) |
| OpenSSL can't be cross-compiled for Android | `reqwest` uses rustls on Android |
| The opener plugin only opens URLs on mobile, so downloaded files could not be opened | Android: the file is opened with the app the user picks, from the system's "open with" list (`plugins/android-native`). iOS: the file is previewed in Quick Look, the system viewer, whose Share button has "Open in…" and "Save to Files" (`plugins/ios-native`) |
| No user-visible Downloads folder, no folder picker | Files go to the app's own storage; the "Download folder" setting is hidden |
| Browser SSO returns through `moodledesk://` | The scheme is registered for Android and iOS in `tauri.conf.json` |
| The system can close the app while the browser login is open | The login attempt is saved on disk and read back when the link returns. A link that starts the app from scratch is not delivered as an event, so the app asks for it at startup (`get_current`). The handler runs off the UI thread, which would otherwise freeze Android |
| Self-update prompts break store rules | The GitHub update check does nothing on mobile |
| Sidebar, `100vh`, notches | Bottom bar under 600 px, `100dvh`, `viewport-fit=cover` and safe-area insets |
| No hover or right-click on touch | Course menu button always visible, 44 px touch targets |
| Android back button closed the app | Each open course is a history entry, so back goes up one level (`src/courseStack.ts`) |
| Sign out lived in the sidebar | On phones it is in Settings → Account |

## Known limits

- **Not run on a device or emulator.** The desktop build and tests pass. The Android-only and iOS-only Rust (the two plugins, Keystore storage, file opening) type-checks against the real Android and iOS targets. The Kotlin and the Swift only parse: nobody has run Gradle or Xcode on them. The layout was rendered in WebKit at phone, tablet and desktop sizes.
- **Files stay inside the app.** They are removed on uninstall and are not shown in the Files app. Opening one hands a copy to another app, and the copy stays in the app's cache until Android clears it.
- **A login attempt does not expire.** It is replaced by the next attempt and deleted when its callback arrives. The passport in it is random and is only useful together with a callback signed by your Moodle site.
- **Stores need a privacy policy link.** Use `PRIVACY.md`, and update it if a store build adds anything.
