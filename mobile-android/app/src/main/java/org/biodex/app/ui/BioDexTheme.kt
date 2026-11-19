package org.biodex.app.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

object BioDexColors {
    val Background = Color(0xFF0A1510)
    val Surface = Color(0xFF13241C)
    val SurfaceElevated = Color(0xFF1C3529)
    val Primary = Color(0xFF34D399)
    val OnPrimary = Color(0xFF052014)
    val TextPrimary = Color(0xFFF1F5F9)
    val TextSecondary = Color(0xFF94A3B8)
    val Warning = Color(0xFFF59E0B)
    val Danger = Color(0xFFEF4444)

    fun rarityColor(rarity: String): Color = when (rarity.lowercase()) {
        "uncommon" -> Color(0xFF34D399)
        "rare" -> Color(0xFF38BDF8)
        "epic" -> Color(0xFFA78BFA)
        "mythic" -> Color(0xFFF59E0B)
        else -> Color(0xFF94A3B8)
    }
}

private val DarkBioDexScheme = darkColorScheme(
    primary = BioDexColors.Primary,
    onPrimary = BioDexColors.OnPrimary,
    background = BioDexColors.Background,
    onBackground = BioDexColors.TextPrimary,
    surface = BioDexColors.Surface,
    onSurface = BioDexColors.TextPrimary,
    surfaceVariant = BioDexColors.SurfaceElevated,
    onSurfaceVariant = BioDexColors.TextSecondary,
    error = BioDexColors.Danger,
)

@Composable
fun BioDexTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = DarkBioDexScheme,
        content = content,
    )
}
