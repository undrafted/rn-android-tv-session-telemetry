package com.rnandroidtvsessiontelemetry.tvfixture

import android.app.Application
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.defaults.DefaultReactHost.getDefaultReactHost

class MainApplication : Application(), ReactApplication {

  // No manual @rn-android-tv-session-telemetry/react-native registration needed: the library exposes
  // exactly one ReactPackage (RnAndroidTvSessionTelemetryPackage), which RN's autolinking discovers on
  // its own - autolinking only finds one `*Package.{java,kt}` file per npm dependency, so this
  // only works because the library keeps to one.
  override val reactHost: ReactHost by lazy {
    getDefaultReactHost(context = applicationContext, packageList = PackageList(this).packages)
  }

  override fun onCreate() {
    super.onCreate()
    loadReactNative(this)
  }
}
