package app.keyflow.mobile.ui.screens.settings

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import android.view.autofill.AutofillManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.getSystemService
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.biometric.BiometricVaultUnlocker
import app.keyflow.mobile.data.AppPreferences
import app.keyflow.mobile.data.AppTheme
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.ui.components.PasswordField

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    vaultRepository: VaultRepository,
    preferences: AppPreferences,
    biometricUnlocker: BiometricVaultUnlocker,
    activity: FragmentActivity,
    onBack: () -> Unit,
    onLocked: () -> Unit,
) {
    val viewModel: SettingsViewModel = viewModel(
        factory = viewModelFactory { initializer { SettingsViewModel(vaultRepository, preferences, biometricUnlocker) } },
    )
    val state by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    var quickUnlockConfirmPassword by remember { mutableStateOf<String?>(null) }
    var isAutofillEnabled by remember { mutableStateOf(false) }

    fun refreshAutofillStatus() {
        val afm = context.getSystemService<AutofillManager>()
        isAutofillEnabled = afm?.hasEnabledAutofillServices() == true
    }
    LaunchedEffect(Unit) { refreshAutofillStatus() }

    val autofillLauncher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        refreshAutofillStatus()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            SettingsSection("Appearance") {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    AppTheme.entries.forEach { theme ->
                        OutlinedButton(onClick = { viewModel.setTheme(theme) }) {
                            Text(theme.name.lowercase().replaceFirstChar { it.uppercase() })
                        }
                    }
                }
            }

            SettingsSection("Autofill") {
                Text(
                    "Let KeyFlow fill your saved logins into other apps and websites, only after you unlock it.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Button(
                    onClick = {
                        val intent = Intent(Settings.ACTION_REQUEST_SET_AUTOFILL_SERVICE, Uri.parse("package:${context.packageName}"))
                        autofillLauncher.launch(intent)
                    },
                    enabled = !isAutofillEnabled,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                ) {
                    Text(if (isAutofillEnabled) "Autofill is enabled" else "Enable KeyFlow Autofill")
                }
            }

            SettingsSection("Locking") {
                Text("Auto-lock after ${state.autoLockMinutes} minute(s) of inactivity")
                Slider(
                    value = state.autoLockMinutes.toFloat(),
                    onValueChange = { viewModel.setAutoLockMinutes(it.toInt()) },
                    valueRange = 0f..30f,
                    steps = 29,
                )
                Button(onClick = { viewModel.lock(); onLocked() }, modifier = Modifier.fillMaxWidth()) {
                    Text("Lock KeyFlow now")
                }
            }

            SettingsSection("Clipboard") {
                Text("Clear clipboard after copying a secret: ${state.clipboardClearSeconds}s")
                Slider(
                    value = state.clipboardClearSeconds.toFloat(),
                    onValueChange = { viewModel.setClipboardClearSeconds(it.toInt()) },
                    valueRange = 0f..120f,
                    steps = 23,
                )
            }

            SettingsSection("Quick unlock") {
                Text(
                    "Unlock KeyFlow with your fingerprint or face instead of typing your master password. " +
                        "Anyone able to unlock this device can then unlock KeyFlow too.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (!state.biometricsAvailable) {
                    Text("No biometrics are set up on this device.", color = MaterialTheme.colorScheme.error)
                } else {
                    Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                        Text("Enable quick unlock", modifier = Modifier.weight(1f))
                        Switch(
                            checked = state.quickUnlockActuallyEnabled,
                            onCheckedChange = { enabling ->
                                if (enabling) quickUnlockConfirmPassword = "" else viewModel.disableQuickUnlock()
                            },
                        )
                    }
                    if (quickUnlockConfirmPassword != null) {
                        PasswordField(
                            value = quickUnlockConfirmPassword ?: "",
                            onValueChange = { quickUnlockConfirmPassword = it },
                            label = "Confirm master password",
                        )
                        Button(
                            onClick = {
                                viewModel.enableQuickUnlock(activity, quickUnlockConfirmPassword.orEmpty())
                                quickUnlockConfirmPassword = null
                            },
                            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                        ) {
                            Text("Confirm")
                        }
                    }
                }
            }

            SettingsSection("Change master password") {
                PasswordField(value = state.newPassword, onValueChange = viewModel::onNewPasswordChange, label = "New master password")
                PasswordField(
                    value = state.confirmPassword,
                    onValueChange = viewModel::onConfirmPasswordChange,
                    label = "Confirm new master password",
                    supportingText = state.changePasswordMessage,
                    isError = state.changePasswordMessage != null && state.changePasswordMessage != "Master password changed.",
                )
                Button(onClick = viewModel::changeMasterPassword, modifier = Modifier.fillMaxWidth()) {
                    Text("Change master password")
                }
            }

            SettingsSection("Export") {
                Text(
                    "Exported data contains your passwords in plain text. Only share it somewhere secure, and delete it once you're done.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Button(onClick = viewModel::exportJson, modifier = Modifier.fillMaxWidth()) {
                    Text("Export vault as JSON")
                }
            }
        }
    }

    state.exportedJson?.let { json ->
        ExportDialog(json = json, onDismiss = viewModel::clearExportedJson)
    }
}

@Composable
private fun SettingsSection(title: String, content: @Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        content()
        HorizontalDivider(modifier = Modifier.padding(top = 8.dp))
    }
}

@Composable
private fun ExportDialog(json: String, onDismiss: () -> Unit) {
    val context = LocalContext.current
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Export vault") },
        text = {
            Column {
                Text("This device's share sheet will receive your vault as plain-text JSON. Only send it somewhere you trust.")
            }
        },
        confirmButton = {
            Button(onClick = {
                val intent = Intent(Intent.ACTION_SEND).apply {
                    type = "application/json"
                    putExtra(Intent.EXTRA_TEXT, json)
                }
                context.startActivity(Intent.createChooser(intent, "Share KeyFlow export"))
                onDismiss()
            }) { Text("Share") }
        },
        dismissButton = {
            OutlinedButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
