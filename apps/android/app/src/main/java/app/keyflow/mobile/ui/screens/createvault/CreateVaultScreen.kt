package app.keyflow.mobile.ui.screens.createvault

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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.ui.components.PasswordField

@Composable
fun CreateVaultScreen(vaultRepository: VaultRepository, onCreated: () -> Unit) {
    val viewModel: CreateVaultViewModel = viewModel(
        factory = viewModelFactory { initializer { CreateVaultViewModel(vaultRepository) } },
    )
    val state by viewModel.uiState.collectAsState()

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().padding(24.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Text("Create your vault", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Choose a master password. There is no way to recover it if you forget it — write it down somewhere safe if you need to.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp, bottom = 24.dp),
            )
            PasswordField(
                value = state.password,
                onValueChange = viewModel::onPasswordChange,
                label = "Master password",
                isError = state.errorMessage != null,
            )
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(top = 12.dp))
            PasswordField(
                value = state.confirmPassword,
                onValueChange = viewModel::onConfirmPasswordChange,
                label = "Confirm master password",
                isError = state.errorMessage != null,
                supportingText = state.errorMessage,
            )
            Button(
                onClick = { viewModel.submit(onCreated) },
                enabled = !state.isSubmitting,
                modifier = Modifier.fillMaxWidth().padding(top = 20.dp),
            ) {
                if (state.isSubmitting) {
                    CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp), strokeWidth = 2.dp)
                }
                Text("Create vault")
            }
        }
    }
}
