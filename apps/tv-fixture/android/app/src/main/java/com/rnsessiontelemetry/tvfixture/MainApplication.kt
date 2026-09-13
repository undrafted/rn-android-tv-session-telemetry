package com.rnsessiontelemetry.tvfixture

import android.app.Application
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.defaults.DefaultReactHost.getDefaultReactHost
import com.rnsessiontelemetry.reactnative.FrameTimingPackage
import com.rnsessiontelemetry.reactnative.SessionWriterPackage

class MainApplication : Application(), ReactApplication {

  override val reactHost: ReactHost by lazy {
    getDefaultReactHost(
      context = applicationContext,
      packageList =
        PackageList(this).packages.apply {
          // Registered manually rather than relying on autolinking discovery, per this file's
          // own guidance - a legacy (non-Turbo) NativeModule package, kept simple since this is
          // a first native prototype (public hooks only, no Codegen).
          add(FrameTimingPackage())
          add(SessionWriterPackage())
        },
    )
  }

  override fun onCreate() {
    super.onCreate()
    loadReactNative(this)
  }
}
