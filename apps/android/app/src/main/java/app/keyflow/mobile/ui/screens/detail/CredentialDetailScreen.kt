package app.keyflow.mobile.ui.screens.detail

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.ui.components.PasswordField
import app.keyflow.mobile.util.ClipboardUtil

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CredentialDetailScreen(vaultRepository: VaultRepository, credentialId: String?, onDone: () -> Unit) {
    val viewModel: CredentialDetailViewModel = viewModel(
        factory = viewModelFactory { initializer { CredentialDetailViewModel(vaultRepository, credentialId) } },
    )
    val state by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var showDeleteConfirm by remember { mutableStateOf(false) }

    LaunchedEffect(state.saved, state.deleted) {
        if (state.saved || state.deleted) onDone()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (credentialId == null) "Add login" else "Edit login") },
                navigationIcon = {
                    IconButton(onClick = onDone) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") }
                },
                actions = {
                    if (credentialId != null) {
                        IconButton(onClick = { showDeleteConfirm = true }) {
                            Icon(Icons.Filled.Delete, contentDescription = "Delete login")
                        }
                    }
                },
            )
        },
    ) { padding ->
        if (state.isLoading) {
            Column(modifier = Modifier.fillMaxSize().padding(padding), verticalArrangement = Arrangement.Center) {
                CircularProgressIndicator(modifier = Modifier.padding(horizontal = 24.dp))
            }
            return@Scaffold
        }
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            OutlinedTextField(value = state.name, onValueChange = viewModel::onNameChange, label = { Text("Name") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
            OutlinedTextField(value = state.url, onValueChange = viewModel::onUrlChange, label = { Text("Website URL") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
            Row(modifier = Modifier.fillMaxWidth()) {
                OutlinedTextField(value = state.username, onValueChange = viewModel::onUsernameChange, label = { Text("Username") }, modifier = Modifier.weight(1f), singleLine = true)
                IconButton(onClick = { ClipboardUtil.copyWithAutoClear(context, "username", state.username, 0, scope) }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = "Copy username")
                }
            }
            Row(modifier = Modifier.fillMaxWidth()) {
                PasswordField(value = state.password, onValueChange = viewModel::onPasswordChange, label = "Password", modifier = Modifier.weight(1f))
                IconButton(onClick = { ClipboardUtil.copyWithAutoClear(context, "password", state.password, 30, scope) }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = "Copy password")
                }
            }
            OutlinedTextField(
                value = state.notes,
                onValueChange = viewModel::onNotesChange,
                label = { Text("Notes") },
                modifier = Modifier.fillMaxWidth().height(100.dp),
            )
            Row {
                Checkbox(checked = state.favorite, onCheckedChange = viewModel::onFavoriteChange)
                Text("Favorite", modifier = Modifier.padding(top = 12.dp))
            }
            state.errorMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.error)
            }
            Spacer(modifier = Modifier.height(4.dp))
            androidx.compose.material3.Button(onClick = viewModel::save, enabled = !state.isSaving, modifier = Modifier.fillMaxWidth()) {
                if (state.isSaving) CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp), strokeWidth = 2.dp)
                Text("Save")
            }
        }
    }

    if (showDeleteConfirm) {
        AlertDialog(
            onDismissRequest = { showDeleteConfirm = false },
            title = { Text("Delete this login?") },
            text = { Text("This can't be undone.") },
            confirmButton = {
                TextButton(onClick = { showDeleteConfirm = false; viewModel.delete() }) { Text("Delete") }
            },
            dismissButton = {
                TextButton(onClick = { showDeleteConfirm = false }) { Text("Cancel") }
            },
        )
    }
}
