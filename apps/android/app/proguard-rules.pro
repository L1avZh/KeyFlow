# KeyFlow release R8/ProGuard rules.
#
# The rule below is not optional: UniFFI's Kotlin bindings load the Rust
# library via JNA and use reflection to bind native methods on
# `uniffi.keyflow_mobile.*`. Without keeping that package, R8 renames or
# strips those classes and every call into the vault engine fails at
# runtime in release builds only — the kind of bug that "works on my
# debug build" hides completely. See uniffi's own Android-consumer docs.
-keep class uniffi.keyflow_mobile.** { *; }
-keepclassmembers class uniffi.keyflow_mobile.** { *; }

# JNA itself relies on reflection over its own native-mapping classes.
-keep class com.sun.jna.** { *; }
-keepclassmembers class com.sun.jna.** { *; }
-dontwarn com.sun.jna.**

# Jetpack Compose compiler already ships consumer ProGuard rules via its
# AARs; nothing project-specific needed beyond keeping our own FFI/JNA
# boundary above. Kotlin coroutines/Compose runtime metadata is preserved
# automatically by the AGP default rules for -keepattributes Signature,
# *Annotation*, EnclosingMethod (applied by the plugin already).
