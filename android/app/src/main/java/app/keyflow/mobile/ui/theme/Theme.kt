package app.keyflow.mobile.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import app.keyflow.mobile.data.AppTheme

private val LightColors = lightColorScheme(
    primary = KfIndigo,
    secondary = KfTeal,
    tertiary = KfTeal,
    background = KfBgLight,
    surface = KfBgElevatedLight,
    surfaceVariant = KfBgSunkenLight,
    onBackground = KfTextLight,
    onSurface = KfTextLight,
    error = KfDanger,
)

private val DarkColors = darkColorScheme(
    primary = KfIndigo,
    secondary = KfTeal,
    tertiary = KfTeal,
    background = KfBgDark,
    surface = KfBgElevatedDark,
    surfaceVariant = KfBgSunkenDark,
    onBackground = KfTextDark,
    onSurface = KfTextDark,
    error = KfDanger,
)

@Composable
fun KeyFlowTheme(appTheme: AppTheme, content: @Composable () -> Unit) {
    val dark = when (appTheme) {
        AppTheme.SYSTEM -> isSystemInDarkTheme()
        AppTheme.LIGHT -> false
        AppTheme.DARK -> true
    }
    MaterialTheme(
        colorScheme = if (dark) DarkColors else LightColors,
        typography = KeyFlowTypography,
        content = content,
    )
}
