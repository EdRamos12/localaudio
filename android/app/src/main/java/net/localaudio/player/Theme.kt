package net.localaudio.player

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val DarkColors = darkColorScheme(
    primary = Color(0xFF5EEAD4),
    onPrimary = Color(0xFF00382F),
    primaryContainer = Color(0xFF134E48),
    onPrimaryContainer = Color(0xFFB2F5EA),
    secondary = Color(0xFFA5B4FC),
    onSecondary = Color(0xFF1E1B4B),
    secondaryContainer = Color(0xFF232E38),
    onSecondaryContainer = Color(0xFFE6EDF3),
    tertiary = Color(0xFFFBBF24),
    error = Color(0xFFFCA5A5),
    errorContainer = Color(0xFF4C1D1D),
    onErrorContainer = Color(0xFFFECACA),
    background = Color(0xFF0D1117),
    onBackground = Color(0xFFE6EDF3),
    surface = Color(0xFF0D1117),
    onSurface = Color(0xFFE6EDF3),
    surfaceVariant = Color(0xFF1F2933),
    onSurfaceVariant = Color(0xFF9AA7B4),
    surfaceContainerLowest = Color(0xFF0A0E13),
    surfaceContainerLow = Color(0xFF131A21),
    surfaceContainer = Color(0xFF161E26),
    surfaceContainerHigh = Color(0xFF1C252E),
    surfaceContainerHighest = Color(0xFF232E38),
    outline = Color(0xFF3A4652),
    outlineVariant = Color(0xFF26313B),
)

private val LightColors = lightColorScheme(
    primary = Color(0xFF00796B),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFB2F5EA),
    onPrimaryContainer = Color(0xFF00382F),
    secondary = Color(0xFF4F46E5),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFE2E8ED),
    onSecondaryContainer = Color(0xFF111827),
    tertiary = Color(0xFFB45309),
    error = Color(0xFFB91C1C),
    errorContainer = Color(0xFFFEE2E2),
    onErrorContainer = Color(0xFF7F1D1D),
    background = Color(0xFFF5F7F9),
    onBackground = Color(0xFF111827),
    surface = Color(0xFFF5F7F9),
    onSurface = Color(0xFF111827),
    surfaceVariant = Color(0xFFE5EAEE),
    onSurfaceVariant = Color(0xFF52606D),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF9FAFB),
    surfaceContainer = Color.White,
    surfaceContainerHigh = Color(0xFFEFF3F6),
    surfaceContainerHighest = Color(0xFFE6EBEF),
    outline = Color(0xFFC3CCD4),
    outlineVariant = Color(0xFFDDE3E8),
)

private val AppTypography = Typography().let { base ->
    base.copy(
        headlineMedium = base.headlineMedium.copy(fontWeight = FontWeight.SemiBold, letterSpacing = (-0.5).sp),
        titleLarge = base.titleLarge.copy(fontWeight = FontWeight.SemiBold),
        titleMedium = base.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        labelLarge = base.labelLarge.copy(fontWeight = FontWeight.SemiBold),
        labelSmall = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.8.sp),
    )
}

private val AppShapes = Shapes(
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

@Composable
fun LocalAudioTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) DarkColors else LightColors,
        typography = AppTypography,
        shapes = AppShapes,
        content = content,
    )
}
