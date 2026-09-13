package com.rnsessiontelemetry.reactnative

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.uimanager.ViewManager

// The single ReactPackage for this whole library. RN's autolinking discovers exactly one
// `*Package.{java,kt}` file per npm dependency (it globs for the pattern and takes the first
// match - see @react-native-community/cli-config-android's findPackageClassName.js) - so with
// three separate package files (one per native module) a host app's MainApplication had to
// register two of them by hand, or they silently did nothing. One package returning every
// module fixes that: a host app needs zero native code to link this library at all.
class RnSessionTelemetryPackage : ReactPackage {
  override fun createNativeModules(reactContext: ReactApplicationContext): List<NativeModule> =
      listOf(
          FrameTimingModule(reactContext),
          SessionWriterModule(reactContext),
          GlobalFocusModule(reactContext),
      )

  override fun createViewManagers(reactContext: ReactApplicationContext): List<ViewManager<*, *>> =
      emptyList()
}
