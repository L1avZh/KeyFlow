package app.keyflow.mobile

import android.app.Application
import app.keyflow.mobile.data.AppPreferences
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.biometric.BiometricVaultUnlocker

/**
 * Manual, dependency-injection-framework-free composition root. The app
 * is small enough (one process, no multi-module build) that Hilt/Dagger
 * would be pure ceremony — every dependency below is a plain constructor
 * call, wired once here and read from [KeyFlowApplication.from].
 */
class KeyFlowApplication : Application() {

    lateinit var preferences: AppPreferences
        private set
    lateinit var vaultRepository: VaultRepository
        private set
    lateinit var biometricUnlocker: BiometricVaultUnlocker
        private set

    override fun onCreate() {
        super.onCreate()
        preferences = AppPreferences(this)
        vaultRepository = VaultRepository(vaultFilePath = filesDir.resolve("vault.keyflow").absolutePath)
        biometricUnlocker = BiometricVaultUnlocker(this)
    }

    companion object {
        fun from(context: android.content.Context): KeyFlowApplication =
            context.applicationContext as KeyFlowApplication
    }
}
