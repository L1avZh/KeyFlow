package app.keyflow.mobile.ui.screens.unlock

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import app.keyflow.mobile.biometric.BiometricVaultUnlocker
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.ui.components.PasswordField

@Composable
fun UnlockScreen(
    vaultRepository: VaultRepository,
    biometricUnlocker: BiometricVaultUnlocker,
    activity: FragmentActivity,
    onUnlocked: () -> Unit,
) {
    val viewModel: UnlockViewModel = viewModel(
        factory = viewModelFactory { initializer { UnlockViewModel(vaultRepository, biometricUnlocker) } },
    )
    val state by viewModel.uiState.collectAsState()

    LaunchedEffect(Unit) {
        if (viewModel.canOfferBiometrics) viewModel.unlockWithBiometrics(activity) { onUnlocked() }
    }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(
                Icons.Filled.VpnKey,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp),
            )
            Text("Unlock KeyFlow", style = MaterialTheme.typography.headlineMedium)
            Spacer(modifier = Modifier.padding(top = 24.dp))
            PasswordField(
                value = state.password,
                onValueChange = viewModel::onPasswordChange,
                label = "Master password",
                isError = state.errorMessage != null,
                supportingText = state.errorMessage,
            )
            Button(
                onClick = { viewModel.unlockWithPassword { onUnlocked() } },
                enabled = !state.isSubmitting,
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
            ) {
                if (state.isSubmitting) CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp), strokeWidth = 2.dp)
                Text("Unlock")
            }
            if (viewModel.canOfferBiometrics) {
                OutlinedButton(
                    onClick = { viewModel.unlockWithBiometrics(activity) { onUnlocked() } },
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                ) {
                    Icon(Icons.Filled.Fingerprint, contentDescription = null, modifier = Modifier.padding(end = 8.dp))
                    Text("Use biometrics")
                }
            }
        }
    }
}
