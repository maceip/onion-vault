# The JNI entry points of Trap are called from the native code.
-keep class dev.onionvault.OvNative { *; }
-keep class dev.onionvault.OvBridge { *; }
-keepclasseswithmembernames class * {
    native <methods>;
}
