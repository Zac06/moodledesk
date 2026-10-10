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

Commit `src-tauri/gen/android` and `src-tauri/gen/apple` after the first `init`. After `android init`, set `android:allowBackup="false"` on `<application>` in `gen/android/app/src/main/AndroidManifest.xml`, otherwise Android's cloud backup copies the saved login. For iOS set your team in `APPLE_DEVELOPMENT_TEAM` (or `bundle.iOS.developmentTeam`).

## What had to change, and why

| Problem on a phone | What the branch does |
| --- | --- |
| No app entry point | `run()` is a Tauri mobile entry point |
| Single-instance plugin is desktop-only | Only registered (and only compiled) on desktop |
| `keyring` has no Android backend | Android keeps the login in a file in the app's private storage (`secret` in `lib.rs`); iOS still uses the Keychain |
| OpenSSL can't be cross-compiled for Android | `reqwest` uses rustls on Android |
| The opener plugin only opens URLs on mobile, so downloaded files could not be opened | A saved file is handed to the system share sheet (`tauri-plugin-sharekit`) |
| No user-visible Downloads folder, no folder picker | Files go to the app's own storage; the "Download folder" setting is hidden |
| Browser SSO returns through `moodledesk://` | The scheme is registered for Android and iOS in `tauri.conf.json` |
| Self-update prompts break store rules | The GitHub update check does nothing on mobile |
| Sidebar, `100vh`, notches | Bottom bar under 600 px, `100dvh`, `viewport-fit=cover` and safe-area insets |
| No hover or right-click on touch | Course menu button always visible, 44 px touch targets |
| Android back button closed the app | Each open course is a history entry, so back goes up one level (`src/courseStack.ts`) |
| Sign out lived in the sidebar | On phones it is in Settings → Account |

## Known limits

- **Not run on a device or emulator.** The Rust code was compile-checked on desktop and, with the platform switches flipped, against the real share plugin and the Android storage code. The layout was rendered in WebKit at phone, tablet and desktop sizes. The native Android and iOS builds themselves have not been run.
- **Opening a file shows the share sheet, not a viewer.** On Android that lists apps that accept the file (Drive, Files, mail) rather than PDF viewers. A proper "open with" needs a small Kotlin/Swift plugin and the Android SDK to test it.
- **Files stay inside the app.** They are removed on uninstall and are not shown in the Files app. Use the share sheet to export one.
- **The Android login is not hardware-backed.** Move it to the Android Keystore before a Play Store release.
- **Logging in through the browser can fail if the system closes the app meanwhile.** The login attempt is kept in memory only.
- **Stores need a privacy policy link.** Use `PRIVACY.md`, and update it if a store build adds anything.
