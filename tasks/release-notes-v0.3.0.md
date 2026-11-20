Carrova 0.3.0 links your computers and your Android phone. Besides moving app data between computers, Carrova can now keep devices on the same network working together: share the clipboard and files, hand off links, ring a lost phone, and see your phone's notifications, texts and calls on your laptop.

Everything stays on your local network, and you choose what each device may do.

## Linked computers and phones

- **Add a device:** pair an Android 13+ phone by scanning a QR code, or another computer with a short code. Before anything is saved, you check the other device's key fingerprint.
- **Clipboard:** copy on a computer, paste on your other devices. On the phone, send a copy with the "Send clipboard" tile or notification action (Android lets only the open app read the clipboard). Items the source app marks sensitive, such as password manager copies, are skipped. The last 20 received items are kept in memory; saving them to disk is opt-in.
- **Files:** send files between linked devices. If a device leaves mid-transfer, Retry sends the file again from the start.
- **Handoff:** open a link from your phone on your computer, or the other way round.
- **Ring:** make a linked phone ring at full volume, even in silent mode.
- **Per-device features:** every feature has to be allowed on both devices, and you can turn each one off for a single device.
- **Command line:** `carrova devices`, `link`, `clip`, `file` and `handoff`.

## Your phone on your laptop

- **Notifications:** your phone's notifications appear on the laptop. Reply, mark as read, or open them on the phone. You choose which apps are forwarded, and one-time codes are hidden by default. Pop-ups pause while your screen is locked or while you're away, if you ask them to.
- **Messages:** read and send texts from the laptop. The phone shares the last 30 days, the last year, or everything, as you choose.
- **Calls:** see who's calling, answer, decline or end calls, and take the call audio on your laptop over Bluetooth. A setup guide walks you through the audio part.
- **Dual SIM:** choose which SIM to text or call from.
- **Calls page:** search your call log and dial from the laptop.

## Android app

The Android app is now attached to the release as a signed APK (`Carrova_0.3.0_android-arm64.apk`, Android 13+, 64-bit ARM). Install it on your phone, then add the phone from Carrova on your computer. The phone app asks for each permission only when you turn on the feature that needs it.

## Fixes

- A notification no longer pops up again after you reply to it, or when you switch pages.
- One slow laptop no longer holds up phone updates to your other computers.
- Turning a feature off now stops its updates at once, including ones already on the way.
- A text no longer shows twice while the phone is still saving it.
- Device names you set yourself are kept after upgrading.

## Notes

- Desktop builds as in 0.2.3: Linux (AppImage, `.deb`, `.rpm`; x86_64 and arm64), macOS (`.dmg`) and Windows (`.msi`, setup `.exe`). Existing installs update in place through the built-in updater.
- Phone features need the Android app from this release on the phone.
