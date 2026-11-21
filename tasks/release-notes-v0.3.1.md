Carrova 0.3.1 lets the Android app update itself, shows live progress while the desktop app updates, and fixes problems found linking a real phone to 0.3.0.

## Updates

- **Android app updates itself:** the phone app checks GitHub releases for a newer version, downloads it (Wi-Fi only unless you allow mobile data) and hands it to Android's installer. A cut download picks up where it stopped. The APK must be signed with Carrova's release key, checked before and again just before install; anything else is deleted. Find it under Settings → Updates on the phone, or on the welcome screen before you link a computer.
- **Desktop update progress:** installing an update now shows the download size, speed and time left, and you can cancel it. If a move or a call is running, the install waits until it ends, or you can install anyway. Settings → Updates shows the version you are running.

## Fixes

- The first pairing with a phone now lists the device right away, so "Choose what to share" appears without pairing twice.
- Choices made in "Choose what to share" are all saved; Messages and Calls no longer stay off.
- Messages and Calls say why they show nothing (turned off on the phone, phone away, connecting, loading) instead of "Couldn't reach".
- A file a device offers raises a pop-up and a bell request, so it no longer expires unseen while the window is hidden; a failed send shows one note instead of two.
- The call window no longer opens blank in the AppImage (the AppImage now bundles the audio framework, so it is larger).
- Toasts on the phone use its full width.

## Notes

- **Phones on 0.3.0 need one manual update:** 0.3.0 has no in-app updater, so install `Carrova_0.3.1_android-arm64.apk` from this release by hand once (Android 13+, 64-bit ARM). Later versions arrive through the app.
- Desktop builds as in 0.3.0: Linux (AppImage, `.deb`, `.rpm`; x86_64 and arm64), macOS (`.dmg`) and Windows (`.msi`, setup `.exe`). Existing installs update in place through the built-in updater.
