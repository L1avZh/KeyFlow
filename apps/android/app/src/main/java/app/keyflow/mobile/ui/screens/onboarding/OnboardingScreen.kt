package app.keyflow.mobile.ui.screens.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PhonelinkLock
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

private data class OnboardingPoint(val icon: ImageVector, val title: String, val body: String)

private val points = listOf(
    OnboardingPoint(
        icon = Icons.Filled.VpnKey,
        title = "One vault for every login",
        body = "Save your usernames and passwords once, then fill them back in on this device whenever you need them.",
    ),
    OnboardingPoint(
        icon = Icons.Filled.Lock,
        title = "Everything stays on this device",
        body = "Your vault is encrypted and stored only on your phone. KeyFlow has no server and never sends your passwords anywhere.",
    ),
    OnboardingPoint(
        icon = Icons.Filled.Shield,
        title = "Locked behind one master password",
        body = "A single master password unlocks your vault. Choose one you'll remember — there is no password reset, by design.",
    ),
    OnboardingPoint(
        icon = Icons.Filled.PhonelinkLock,
        title = "Unlock with your fingerprint or face",
        body = "After you set up your vault, you can enable quick unlock so you don't have to type your master password every time.",
    ),
)

@Composable
fun OnboardingScreen(onDone: () -> Unit) {
    Surface(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().padding(24.dp),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
                Spacer(modifier = Modifier.height(24.dp))
                Text("Welcome to KeyFlow", style = MaterialTheme.typography.headlineMedium)
                points.forEach { point ->
                    Row(verticalAlignment = Alignment.Top) {
                        Icon(point.icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        Column(modifier = Modifier.padding(start = 16.dp)) {
                            Text(point.title, style = MaterialTheme.typography.titleMedium)
                            Text(
                                point.body,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
            Column {
                Button(onClick = onDone, modifier = Modifier.fillMaxWidth()) {
                    Text("Get started")
                }
                Text(
                    "Next, you'll create your vault and choose a master password.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                )
            }
        }
    }
}
