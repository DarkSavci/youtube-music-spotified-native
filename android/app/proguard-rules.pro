# The core's gomobile bindings are called from Go by name.
-keep class go.** { *; }
-keep class dev.darksavci.spotified.core.mobile.** { *; }
# The Python bridge implements the core's interface, which Go calls by name.
-keep class dev.darksavci.spotified.core.** { *; }
# Chaquopy finds Java classes reflectively.
-keep class com.chaquo.python.** { *; }
# kotlinx.serialization keeps its generated serializers.
-keepattributes *Annotation*, InnerClasses
-keepclassmembers class dev.darksavci.spotified.** {
    *** Companion;
}
-keepclasseswithmembers class dev.darksavci.spotified.** {
    kotlinx.serialization.KSerializer serializer(...);
}
