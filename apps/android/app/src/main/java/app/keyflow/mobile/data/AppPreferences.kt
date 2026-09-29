package app.keyflow.mobile.data

import android.content.Context
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore by preferencesDataStore(name = "keyflow_prefs")

enum class AppTheme { SYSTEM, LIGHT, DARK }

/**
 * Non-secret UI/behavior preferences only — never anything about the
 * vault's contents or master password. Quick-unlock's actual secret
 * lives in the Keystore-gated store in [app.keyflow.mobile.biometric.BiometricVaultUnlocker],
 * not here; this only stores the boolean "is it turned on".
 */
class AppPreferences(private val context: Context) {
    private object Keys {
        val THEME = stringPreferencesKey("theme")
        val AUTO_LOCK_MINUTES = intPreferencesKey("auto_lock_minutes")
        val CLIPBOARD_CLEAR_SECONDS = intPreferencesKey("clipboard_clear_seconds")
        val QUICK_UNLOCK_ENABLED = booleanPreferencesKey("quick_unlock_enabled")
        val ONBOARDING_COMPLETE = booleanPreferencesKey("onboarding_complete")
    }

    val theme: Flow<AppTheme> = context.dataStore.data.map { prefs ->
        prefs[Keys.THEME]?.let { runCatching { AppTheme.valueOf(it) }.getOrNull() } ?: AppTheme.SYSTEM
    }

    val autoLockMinutes: Flow<Int> = context.dataStore.data.map { it[Keys.AUTO_LOCK_MINUTES] ?: 5 }

    val clipboardClearSeconds: Flow<Int> = context.dataStore.data.map { it[Keys.CLIPBOARD_CLEAR_SECONDS] ?: 30 }

    val quickUnlockEnabled: Flow<Boolean> = context.dataStore.data.map { it[Keys.QUICK_UNLOCK_ENABLED] ?: false }

    val onboardingComplete: Flow<Boolean> = context.dataStore.data.map { it[Keys.ONBOARDING_COMPLETE] ?: false }

    suspend fun setTheme(theme: AppTheme) {
        context.dataStore.edit { it[Keys.THEME] = theme.name }
    }

    suspend fun setAutoLockMinutes(minutes: Int) {
        context.dataStore.edit { it[Keys.AUTO_LOCK_MINUTES] = minutes }
    }

    suspend fun setClipboardClearSeconds(seconds: Int) {
        context.dataStore.edit { it[Keys.CLIPBOARD_CLEAR_SECONDS] = seconds }
    }

    suspend fun setQuickUnlockEnabled(enabled: Boolean) {
        context.dataStore.edit { it[Keys.QUICK_UNLOCK_ENABLED] = enabled }
    }

    suspend fun setOnboardingComplete(complete: Boolean) {
        context.dataStore.edit { it[Keys.ONBOARDING_COMPLETE] = complete }
    }
}
