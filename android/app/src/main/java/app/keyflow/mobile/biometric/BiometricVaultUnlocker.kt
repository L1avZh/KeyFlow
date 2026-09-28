package app.keyflow.mobile.biometric

import app.keyflow.mobile.R
import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.edit
import androidx.fragment.app.FragmentActivity
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine

/**
 * Android's equivalent of the desktop app's OS-keychain "quick unlock"
 * feature (Settings → Quick unlock in both apps) — same tradeoff,
 * disclosed the same way: this stores the master password so the user
 * doesn't retype it every launch, gated behind a Keystore key that only
 * unwraps after a successful biometric check. Anyone who can unlock this
 * *device* with its own biometrics/PIN can therefore also unlock KeyFlow,
 * exactly like the desktop disclosure says about the OS account.
 *
 * The security boundary is the Android Keystore key itself, not where
 * the ciphertext blob is stored: the key requires
 * [KeyGenParameterSpec.Builder.setUserAuthenticationRequired] and is
 * hardware-backed where the device supports it, so the encrypted master
 * password is unrecoverable without a fresh biometric check regardless
 * of where the ciphertext bytes sit — a plain (not "Encrypted")
 * SharedPreferences file is deliberately sufficient here.
 */
class BiometricVaultUnlocker(private val context: Context) {
    private val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
    private val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    fun canUseBiometrics(): Boolean =
        BiometricManager.from(context).canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG) ==
            BiometricManager.BIOMETRIC_SUCCESS

    fun isEnabled(): Boolean = keyStore.containsAlias(KEY_ALIAS) && prefs.contains(PREF_CIPHERTEXT)

    /**
     * Enables quick unlock: call only right after the user has typed
     * their real master password successfully (e.g. right after
     * [app.keyflow.mobile.data.VaultRepository.unlock] succeeds), so we
     * never persist a password that hasn't actually been verified
     * against the vault.
     */
    suspend fun enable(activity: FragmentActivity, masterPassword: String): Result<Unit> = runCatching {
        val key = generateKey()
        val cipher = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, key) }
        val authenticatedCipher = authenticate(activity, cipher, isEncrypting = true)
        val ciphertext = authenticatedCipher.doFinal(masterPassword.toByteArray(Charsets.UTF_8))
        prefs.edit {
            putString(PREF_CIPHERTEXT, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
            putString(PREF_IV, Base64.encodeToString(authenticatedCipher.iv, Base64.NO_WRAP))
        }
    }

    fun disable() {
        runCatching { keyStore.deleteEntry(KEY_ALIAS) }
        prefs.edit { remove(PREF_CIPHERTEXT); remove(PREF_IV) }
    }

    /** Prompts for biometrics and, on success, returns the stored master password. */
    suspend fun unlock(activity: FragmentActivity): Result<String> = runCatching {
        val ciphertext = Base64.decode(requireNotNull(prefs.getString(PREF_CIPHERTEXT, null)) { "quick unlock not enabled" }, Base64.NO_WRAP)
        val iv = Base64.decode(requireNotNull(prefs.getString(PREF_IV, null)) { "quick unlock not enabled" }, Base64.NO_WRAP)
        val key = keyStore.getKey(KEY_ALIAS, null) as SecretKey
        val cipher = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, iv)) }
        val authenticatedCipher = authenticate(activity, cipher, isEncrypting = false)
        String(authenticatedCipher.doFinal(ciphertext), Charsets.UTF_8)
    }.onFailure {
        // A biometric-invalidating event (new fingerprint enrolled, all
        // biometrics removed) makes the Keystore key permanently unusable
        // — surface that as "quick unlock is off now", not a silent loop
        // of failures, matching the desktop app's own disclosure that
        // this is a convenience feature, not a hard guarantee.
        if (it is android.security.keystore.KeyPermanentlyInvalidatedException) disable()
    }

    private fun generateKey(): SecretKey {
        keyStore.deleteEntry(KEY_ALIAS)
        val spec = KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
            .apply {
                // setUserAuthenticationParameters(timeout, type) — the API
                // that lets us explicitly require BIOMETRIC_STRONG — needs
                // API 30; minSdk here is 26 (required anyway for Autofill).
                // Below API 30, `setUserAuthenticationValidityDurationSeconds(-1)`
                // is the documented equivalent: it requires a fresh
                // per-operation authentication via a `BiometricPrompt`
                // `CryptoObject` (exactly how this key is always used
                // here), which is the same "every use needs biometrics
                // right now" guarantee, just without the ability to name
                // BIOMETRIC_STRONG specifically at the Keystore layer —
                // callers must supply the display-side restriction
                // instead, which BiometricVaultUnlocker.canUseBiometrics()
                // and the BiometricPrompt.PromptInfo below already do via
                // BiometricManager.Authenticators.BIOMETRIC_STRONG.
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
                } else {
                    @Suppress("DEPRECATION")
                    setUserAuthenticationValidityDurationSeconds(-1)
                }
            }
            .setRandomizedEncryptionRequired(true)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE).apply { init(spec) }.generateKey()
    }

    private suspend fun authenticate(activity: FragmentActivity, cipher: Cipher, isEncrypting: Boolean): Cipher =
        suspendCoroutine { continuation ->
            val prompt = BiometricPrompt(
                activity,
                androidx.core.content.ContextCompat.getMainExecutor(activity),
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        val resultCipher = result.cryptoObject?.cipher
                        if (resultCipher != null) continuation.resume(resultCipher)
                        else continuation.resumeWithException(IllegalStateException("biometric result had no cipher"))
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        continuation.resumeWithException(BiometricAuthException(errorCode, errString.toString()))
                    }

                    override fun onAuthenticationFailed() {
                        // A single failed attempt (e.g. unrecognized fingerprint) — the
                        // prompt stays open for a retry; only onAuthenticationError is terminal.
                    }
                },
            )
            val info = BiometricPrompt.PromptInfo.Builder()
                .setTitle(context.getString(R.string.biometric_prompt_title))
                .setSubtitle(context.getString(R.string.biometric_prompt_subtitle))
                .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
                .setNegativeButtonText(if (isEncrypting) "Cancel" else "Use master password")
                .build()
            prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
        }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val KEY_ALIAS = "keyflow_quick_unlock_key"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val PREFS_NAME = "keyflow_quick_unlock"
        private const val PREF_CIPHERTEXT = "ciphertext"
        private const val PREF_IV = "iv"
    }
}

class BiometricAuthException(val errorCode: Int, message: String) : Exception(message)
