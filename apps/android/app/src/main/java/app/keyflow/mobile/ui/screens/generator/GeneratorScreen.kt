package app.keyflow.mobile.ui.screens.generator

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import app.keyflow.mobile.util.ClipboardUtil

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GeneratorScreen(onBack: () -> Unit) {
    val viewModel: GeneratorViewModel = viewModel()
    val state by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Password generator") },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
    ) { padding ->
        Column(modifier = Modifier.fillMaxWidth().padding(padding).padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Card(modifier = Modifier.fillMaxWidth()) {
                Row(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                    Text(
                        state.password,
                        fontFamily = FontFamily.Monospace,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { viewModel.regenerate() }) { Icon(Icons.Filled.Refresh, contentDescription = "Regenerate") }
                    IconButton(onClick = { ClipboardUtil.copyWithAutoClear(context, "password", state.password, 30, scope) }) {
                        Icon(Icons.Filled.ContentCopy, contentDescription = "Copy")
                    }
                }
            }
            Text("${state.strengthLabel} · ~${state.entropyBits.toInt()} bits", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)

            Text("Length: ${state.length}")
            Slider(
                value = state.length.toFloat(),
                onValueChange = { viewModel.onLengthChange(it.toInt()) },
                valueRange = 8f..64f,
                steps = 55,
            )

            ToggleRow("Uppercase (A-Z)", state.uppercase) { viewModel.onToggle(uppercase = it) }
            ToggleRow("Lowercase (a-z)", state.lowercase) { viewModel.onToggle(lowercase = it) }
            ToggleRow("Digits (0-9)", state.digits) { viewModel.onToggle(digits = it) }
            ToggleRow("Symbols (!@#\$...)", state.symbols) { viewModel.onToggle(symbols = it) }
            ToggleRow("Exclude ambiguous characters (0/O, 1/l/I)", state.excludeAmbiguous) { viewModel.onToggle(excludeAmbiguous = it) }
        }
    }
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
        Text(label, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
