# Consumer ProGuard rules for the tur Compose integration.
#
# The JNI bridge (TurNative) declares `external fun`s that are resolved by name
# against `Java_ai_tur_TurNative_*` symbols in libtur_android.so. Keep the
# bridge object, the FrameLoop (passed across JNI as a jobject), and the
# `external` method signatures so the native side can always find them.

-keep class ai.tur.TurNative { *; }
-keep class ai.tur.FrameLoop { *; }
-keep class ai.tur.TurEngine { *; }

# Keep the native method signatures (the `Java_…` symbol names depend on the
# Kotlin method names + parameter types being unchanged).
-keepclasseswithmembernames class * {
    native <methods>;
}
