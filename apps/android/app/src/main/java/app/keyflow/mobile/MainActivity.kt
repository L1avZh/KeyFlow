package app.keyflow.mobile

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.lifecycleScope
import app.keyflow.mobile.data.AppTheme
import app.keyflow.mobile.ui.KeyFlowApp
import app.keyflow.mobile.ui.theme.KeyFlowTheme
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class MainActivity : FragmentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        installSplashScreen()
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Defense in depth: blocks screenshots/screen recording of vault
        // contents and redacts this app's entry in the recent-apps
        // switcher to just its icon, never a snapshot of the last screen
        // (e.g. a revealed password). Applies for the lifetime of the
        // activity, not just while a specific screen is shown, since a
        // credential's password can be revealed on more than one screen.
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)

        val app = KeyFlowApplication.from(this)

        setContent {
            val theme by app.preferences.theme.collectAsState(initial = AppTheme.SYSTEM)
            KeyFlowTheme(appTheme = theme) {
                KeyFlowApp(
                    vaultRepository = app.vaultRepository,
                    preferences = app.preferences,
                    biometricUnlocker = app.biometricUnlocker,
                    activity = this,
                )
            }
        }

        lifecycleScope.launch {
            app.preferences.autoLockMinutes.collectLatest { minutes ->
                autoLockMinutes = minutes
            }
        }
    }

    private var autoLockMinutes: Int = 5

    /**
     * Called by the Android framework on every touch/key event anywhere
     * in this activity — the standard hook for "the user is actively
     * using the app", used here instead of instrumenting every
     * individual screen to reset the auto-lock countdown.
     */
    override fun onUserInteraction() {
        super.onUserInteraction()
        KeyFlowApplication.from(this).vaultRepository.noteUserActivity(autoLockMinutes, lifecycleScope)
    }

    override fun onResume() {
        super.onResume()
        KeyFlowApplication.from(this).vaultRepository.noteUserActivity(autoLockMinutes, lifecycleScope)
    }
}
