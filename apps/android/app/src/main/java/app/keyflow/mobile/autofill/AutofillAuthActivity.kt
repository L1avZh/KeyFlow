package app.keyflow.mobile.autofill

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.view.autofill.AutofillId
import android.view.autofill.AutofillManager
import android.view.autofill.AutofillValue
import android.widget.RemoteViews
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.KeyFlowApplication
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.ui.components.PasswordField
import app.keyflow.mobile.ui.screens.unlock.UnlockViewModel
import app.keyflow.mobile.ui.theme.KeyFlowTheme
import app.keyflow.mobile.data.AppTheme
import uniffi.keyflow_mobile.AutofillMatchRecord

/**
 * Transparent activity [KeyFlowAutofillService] launches (via
 * [FillResponse.Builder.setAuthentication]) only when the vault is
 * locked at fill time. Requires the same master password/biometric
 * unlock as the main app — the autofill framework itself never bypasses
 * that; it only supplies *where* the resulting values are inserted.
 */
class AutofillAuthActivity : FragmentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val origin = intent.getStringExtra(EXTRA_ORIGIN)
        val autofillIds = getAutofillIdsExtra(intent)
        val kinds = intent.getStringArrayExtra(EXTRA_FIELD_KINDS)

        if (origin == null || autofillIds == null || kinds == null || autofillIds.size != kinds.size) {
            setResult(Activity.RESULT_CANCELED)
            finish()
            return
        }
        val fields = autofillIds.indices.map { i -> DetectedField(autofillIds[i], FieldKind.valueOf(kinds[i])) }
        val app = KeyFlowApplication.from(this)

        setContent {
            KeyFlowTheme(appTheme = AppTheme.SYSTEM) {
                AuthScreen(
                    vaultRepository = app.vaultRepository,
                    biometricUnlocker = app.biometricUnlocker,
                    activity = this,
                    onUnlocked = {
                        val matches = app.vaultRepository.findAutofillMatches(origin).getOrNull().orEmpty()
                        finishWithMatches(fields, matches)
                    },
                )
            }
        }
    }

    private fun finishWithMatches(fields: List<DetectedField>, matches: List<AutofillMatchRecord>) {
        val responseBuilder = FillResponse.Builder()
        for (match in matches) {
            val datasetBuilder = Dataset.Builder()
            var any = false
            for (field in fields) {
                val value = when (field.kind) {
                    FieldKind.USERNAME -> match.credential.username
                    FieldKind.PASSWORD -> match.credential.password
                }
                if (value.isEmpty()) continue
                val presentation = RemoteViews(packageName, android.R.layout.simple_list_item_1).apply {
                    setTextViewText(android.R.id.text1, match.credential.name)
                }
                datasetBuilder.setValue(field.autofillId, AutofillValue.forText(value), presentation)
                any = true
            }
            if (any) responseBuilder.addDataset(datasetBuilder.build())
        }
        val result = runCatching { responseBuilder.build() }.getOrNull()
        val replyIntent = Intent().apply {
            putExtra(AutofillManager.EXTRA_AUTHENTICATION_RESULT, result)
        }
        setResult(Activity.RESULT_OK, replyIntent)
        finish()
    }

    companion object {
        private const val EXTRA_ORIGIN = "app.keyflow.mobile.autofill.ORIGIN"
        private const val EXTRA_AUTOFILL_IDS = "app.keyflow.mobile.autofill.AUTOFILL_IDS"
        private const val EXTRA_FIELD_KINDS = "app.keyflow.mobile.autofill.FIELD_KINDS"

        fun intent(context: Context, origin: String, fields: List<DetectedField>): Intent =
            Intent(context, AutofillAuthActivity::class.java).apply {
                putExtra(EXTRA_ORIGIN, origin)
                putExtra(EXTRA_AUTOFILL_IDS, fields.map { it.autofillId }.toTypedArray())
                putExtra(EXTRA_FIELD_KINDS, fields.map { it.kind.name }.toTypedArray())
            }

        // Intent.getParcelableArrayExtra(String, Class<T>) is API 33+
        // only; minSdk here is 26, so the typed overload would throw
        // NoSuchMethodError at runtime on Android 8-12 even though it
        // compiles fine against a higher compileSdk. Fall back to the
        // deprecated untyped overload (still correct, just unchecked)
        // below API 33.
        @Suppress("DEPRECATION")
        private fun getAutofillIdsExtra(intent: Intent): Array<AutofillId>? =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableArrayExtra(EXTRA_AUTOFILL_IDS, AutofillId::class.java)
            } else {
                @Suppress("UNCHECKED_CAST")
                intent.getParcelableArrayExtra(EXTRA_AUTOFILL_IDS) as? Array<AutofillId>
            }
    }
}

@androidx.compose.runtime.Composable
private fun AuthScreen(
    vaultRepository: VaultRepository,
    biometricUnlocker: app.keyflow.mobile.biometric.BiometricVaultUnlocker,
    activity: FragmentActivity,
    onUnlocked: suspend () -> Unit,
) {
    val viewModel: UnlockViewModel = viewModel(
        factory = viewModelFactory { initializer { UnlockViewModel(vaultRepository, biometricUnlocker) } },
    )
    val state by viewModel.uiState.collectAsState()

    LaunchedEffect(Unit) {
        if (viewModel.canOfferBiometrics) viewModel.unlockWithBiometrics(activity, onUnlocked)
    }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("Unlock KeyFlow to fill this", style = MaterialTheme.typography.titleLarge)
            PasswordField(
                value = state.password,
                onValueChange = viewModel::onPasswordChange,
                label = "Master password",
                isError = state.errorMessage != null,
                supportingText = state.errorMessage,
                modifier = Modifier.padding(top = 20.dp),
            )
            Button(
                onClick = { viewModel.unlockWithPassword(onUnlocked) },
                enabled = !state.isSubmitting,
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            ) {
                if (state.isSubmitting) CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp), strokeWidth = 2.dp)
                Text("Unlock and fill")
            }
        }
    }
}
