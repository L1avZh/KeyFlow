# Google Play Data Safety — draft answers

Source answers for Play Console's Data Safety form (Console → your app →
Policy → App content → Data safety). This file is prep material — the
actual submission happens in Play Console itself; nothing here is
transmitted automatically. Re-verify each answer against the live form,
since Play's questionnaire wording/categories change over time.

## Does your app collect or share any of the required user data types?
**No.**

Rationale: KeyFlow has no server, no account system, no analytics SDK,
and no ad SDK (see [`PRIVACY.md`](../../PRIVACY.md)). Every piece of data
the app touches — vault contents, preferences, the optional Keystore-
gated quick-unlock secret — is created by the user, stored only in this
app's private app-storage sandbox on their own device, and never
transmitted off-device by KeyFlow. `network_security_config.xml` also
forbids cleartext traffic app-wide as defense in depth, even though the
app makes no network requests of its own.

## Data types to explicitly mark "not collected"
- Personal info (name, email, address, phone, etc.)
- Financial info
- Health and fitness
- Messages
- Photos and videos
- Audio files
- Files and docs
- Calendar
- Contacts
- App activity (including in-app search history, installed apps, etc.)
- App info and performance (crash logs, diagnostics) — **only if** no
  crash-reporting SDK is added later; if one ever is, this section and
  the "no analytics" claim above must be revisited together.
- Device or other identifiers

## Is all user data encrypted in transit?
Not applicable — no data is ever transmitted.

## Does your app provide a way for users to request data deletion?
Yes, implicitly: uninstalling the app deletes its private storage
(vault file, preferences, Keystore key), and Settings provides in-app
deletion of individual logins or the whole vault. There is no
server-side copy to separately request deletion of.

## Security practices section
- "Data is encrypted in transit": not applicable (no transmission).
- "You can request that data be deleted": Yes (uninstall / in-app delete).
- "Committed to following the Play Families Policy": not applicable
  unless targeting children's category, which KeyFlow does not.

## Notes for whoever fills out the live form
Answer every category as "Data not collected" — do not accept Play
Console's default suggestions without checking them, since some
defaults assume common SDKs (ad networks, crash reporters, analytics)
that KeyFlow deliberately does not include. If a future version adds any
such dependency, this file and the actual Play Console answers must be
updated together, not just one or the other.
