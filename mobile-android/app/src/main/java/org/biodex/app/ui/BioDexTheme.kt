package org.biodex.app.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

object BioDexColors {
    val Background = Color(0xFF0B0B0D)
    val Surface = Color(0xFF151518)
    val SurfaceElevated = Color(0xFF232328)
    val Primary = Color(0xFFF4F4F5)
    val OnPrimary = Color(0xFF0B0B0D)
    val TextPrimary = Color(0xFFFAFAFA)
    val TextSecondary = Color(0xFFA1A1AA)
    val Warning = Color(0xFFD4D4D8)
    val Danger = Color(0xFF71717A)

    fun rarityColor(rarity: String): Color = when (rarity.lowercase()) {
        "uncommon" -> Color(0xFFB4B4BC)
        "rare" -> Color(0xFFD4D4D8)
        "epic" -> Color(0xFFE4E4E7)
        "mythic" -> Color(0xFFFFFFFF)
        else -> Color(0xFF8E8E98)
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
