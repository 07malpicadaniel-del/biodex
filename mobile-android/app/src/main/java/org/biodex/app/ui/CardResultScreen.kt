package org.biodex.app.ui

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.biodex.app.data.CaptureUploadResponse
import org.biodex.app.data.SpeciesCardDto

@Composable
fun CardResultScreen(
    response: CaptureUploadResponse,
    capturedJpegBytes: ByteArray?,
    onScanAnother: () -> Unit,
) {
    val card = response.card
    val borderColor = if (card != null) {
        BioDexColors.rarityColor(card.rarity)
    } else {
        BioDexColors.Warning
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (card != null) {
            MintedSpeciesCard(
                card = card,
                zoneName = response.classification.zoneName,
                capturedJpegBytes = capturedJpegBytes,
                borderColor = borderColor,
            )
        } else {
            DiscardedCaptureNotice(response = response)
        }

        Button(
            onClick = onScanAnother,
            modifier = Modifier
                .fillMaxWidth()
                .height(52.dp),
            shape = RoundedCornerShape(14.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
        ) {
            Text(
                text = "Nueva Captura en Vivo",
                color = BioDexColors.OnPrimary,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

@Composable
private fun MintedSpeciesCard(
    card: SpeciesCardDto,
    zoneName: String?,
    capturedJpegBytes: ByteArray?,
    borderColor: Color,
) {
    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(22.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(2.5.dp, borderColor, RoundedCornerShape(22.dp)),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = card.displayName,
                        style = MaterialTheme.typography.headlineSmall,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = card.scientificName,
                        style = MaterialTheme.typography.bodyMedium,
                        fontStyle = FontStyle.Italic,
                        color = BioDexColors.TextSecondary,
                    )
                }

                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(CircleShape)
                        .background(BioDexColors.Primary),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = card.deployCost.toString(),
                        style = MaterialTheme.typography.titleLarge,
                        color = BioDexColors.OnPrimary,
                        fontWeight = FontWeight.ExtraBold,
                    )
                }
            }

            // Live photo captured via CameraX
            val bitmap = remember(capturedJpegBytes) {
                capturedJpegBytes?.let { bytes ->
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
                }
            }
            if (bitmap != null) {
                Image(
                    bitmap = bitmap,
                    contentDescription = card.displayName,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(220.dp)
                        .clip(RoundedCornerShape(14.dp)),
                )
            }

            // Rarity + Wild/Sanctuary Badges
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                BadgeChip(
                    text = card.rarity.uppercase(),
                    color = borderColor,
                )
                val isSanctuary = card.variant == "sanctuary"
                BadgeChip(
                    text = if (isSanctuary) "SANTUARIO · 85%" else "SALVAJE · 100%",
                    color = if (isSanctuary) BioDexColors.Warning else BioDexColors.Primary,
                )
                BadgeChip(
                    text = card.affinity.uppercase(),
                    color = BioDexColors.TextSecondary,
                )
            }

            if (zoneName != null) {
                Text(
                    text = "Zona verificada: $zoneName",
                    style = MaterialTheme.typography.bodySmall,
                    color = BioDexColors.Warning,
                )
            }

            // 4-Stat Grid
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .background(BioDexColors.SurfaceElevated)
                    .padding(vertical = 12.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                StatCell("HP", card.stats.hp)
                StatCell("ATK", card.stats.attack)
                StatCell("DEF", card.stats.defense)
                StatCell("SPD", card.stats.speed)
            }

            // Moves
            Text(
                text = "Movimientos",
                style = MaterialTheme.typography.titleSmall,
                color = BioDexColors.TextSecondary,
                fontWeight = FontWeight.SemiBold,
            )
            card.moves.forEach { move ->
                Surface(
                    color = BioDexColors.SurfaceElevated,
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = move.name,
                                style = MaterialTheme.typography.titleSmall,
                                color = BioDexColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                            )
                            Text(
                                text = "Energía: ${move.energyCost}" +
                                    (move.tag?.let { " · #$it" } ?: ""),
                                style = MaterialTheme.typography.labelSmall,
                                color = BioDexColors.TextSecondary,
                            )
                        }
                        Text(
                            text = "${move.power} PWR",
                            style = MaterialTheme.typography.titleMedium,
                            color = BioDexColors.Primary,
                            fontWeight = FontWeight.ExtraBold,
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(2.dp))
            Text(
                text = "Pack aplicado: ${card.appliedPack} · Firma hardware P-256 verificada",
                style = MaterialTheme.typography.labelSmall,
                color = BioDexColors.TextSecondary,
            )
        }
    }
}

@Composable
private fun DiscardedCaptureNotice(response: CaptureUploadResponse) {
    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(20.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(2.dp, BioDexColors.Warning, RoundedCornerShape(20.dp)),
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text(
                text = "Captura Fuera de Rango",
                style = MaterialTheme.typography.headlineSmall,
                color = BioDexColors.Warning,
                fontWeight = FontWeight.Bold,
            )
            Text(
                text = "Especie: ${response.identification.scientificName}",
                style = MaterialTheme.typography.bodyMedium,
                color = BioDexColors.TextPrimary,
            )
            Text(
                text = "La coordenada GPS no coincide con un rango natural registrado ni con un santuario verificado en PostGIS.",
                style = MaterialTheme.typography.bodyMedium,
                color = BioDexColors.TextSecondary,
            )
        }
    }
}

@Composable
private fun BadgeChip(text: String, color: Color) {
    Surface(
        color = color.copy(alpha = 0.16f),
        shape = RoundedCornerShape(8.dp),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelSmall,
            color = color,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

@Composable
private fun StatCell(label: String, value: Int) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = BioDexColors.TextSecondary,
        )
        Text(
            text = value.toString(),
            style = MaterialTheme.typography.titleLarge,
            color = BioDexColors.TextPrimary,
            fontWeight = FontWeight.Bold,
        )
    }
}
