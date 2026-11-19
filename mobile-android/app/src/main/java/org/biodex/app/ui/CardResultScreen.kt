package org.biodex.app.ui

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.biodex.app.data.CaptureUploadResponse
import kotlin.math.roundToInt

@Composable
fun CardResultScreen(
    response: CaptureUploadResponse,
    capturedJpegBytes: ByteArray?,
    onScanAnother: () -> Unit,
    onOpenCollection: () -> Unit,
) {
    val context = LocalContext.current
    val isMinted = response.card != null && response.status == "minted"
    val isSanctuary = response.classification.classification == "sanctuary"
    val borderColor = when {
        !isMinted -> BioDexColors.Warning
        isSanctuary -> BioDexColors.Warning
        else -> BioDexColors.Primary
    }
    val naturalistaUrl = BioDexFormatters.naturalistaUrl(
        taxonId = response.identification.taxonId,
        rawUrl = response.naturalistaUrl,
    )

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
        if (isMinted) {
            MintedDiscoveryCard(
                response = response,
                capturedJpegBytes = capturedJpegBytes,
                borderColor = borderColor,
                naturalistaUrl = naturalistaUrl,
                onOpenNaturaLista = { BioDexFormatters.openExternalUrl(context, naturalistaUrl) },
            )
        } else {
            DiscardedCaptureNotice(
                response = response,
                capturedJpegBytes = capturedJpegBytes,
                naturalistaUrl = naturalistaUrl,
                onOpenNaturaLista = { BioDexFormatters.openExternalUrl(context, naturalistaUrl) },
            )
        }

        if (isMinted) {
            OutlinedButton(
                onClick = onOpenCollection,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(50.dp),
                shape = RoundedCornerShape(14.dp),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.MenuBook,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = "Ver en Colección",
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )
            }
        }

        Button(
            onClick = onScanAnother,
            modifier = Modifier
                .fillMaxWidth()
                .height(52.dp),
            shape = RoundedCornerShape(14.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
        ) {
            Icon(
                imageVector = Icons.Default.CameraAlt,
                contentDescription = null,
                tint = BioDexColors.OnPrimary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = "Nueva Captura en Vivo",
                color = BioDexColors.OnPrimary,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

@Composable
private fun MintedDiscoveryCard(
    response: CaptureUploadResponse,
    capturedJpegBytes: ByteArray?,
    borderColor: Color,
    naturalistaUrl: String,
    onOpenNaturaLista: () -> Unit,
) {
    val ident = response.identification
    val isSanctuary = response.classification.classification == "sanctuary"
    val confidencePct = (ident.confidence * 100f).roundToInt().coerceIn(0, 100)

    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(22.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(2.5.dp, borderColor, RoundedCornerShape(22.dp)),
    ) {
        Column(
            modifier = Modifier.padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.CheckCircle,
                    contentDescription = null,
                    tint = borderColor,
                    modifier = Modifier.size(18.dp),
                )
                Text(
                    text = "¡Especie Registrada en tu Colección!",
                    style = MaterialTheme.typography.labelLarge,
                    color = borderColor,
                    fontWeight = FontWeight.Bold,
                )
            }

            Column {
                Text(
                    text = ident.commonName ?: ident.scientificName,
                    style = MaterialTheme.typography.headlineSmall,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    text = ident.scientificName,
                    style = MaterialTheme.typography.bodyMedium,
                    fontStyle = FontStyle.Italic,
                    color = BioDexColors.TextSecondary,
                )
                if (!ident.subspeciesLabel.isNullOrBlank()) {
                    Text(
                        text = "Subespecie detectada (rollup aplicado): ${ident.subspeciesLabel}",
                        style = MaterialTheme.typography.labelSmall,
                        color = BioDexColors.Primary,
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
                    contentDescription = ident.commonName ?: ident.scientificName,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(230.dp)
                        .clip(RoundedCornerShape(14.dp)),
                )
            }

            // Capture Variant + Conservation + Confidence (NO combat info)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                ResultBadgeChip(
                    text = if (isSanctuary) "SANTUARIO" else "SALVAJE",
                    color = if (isSanctuary) BioDexColors.Warning else BioDexColors.Primary,
                )
                ResultBadgeChip(
                    text = "$confidencePct% confianza",
                    color = BioDexColors.Primary,
                )
                response.iucnStatus?.let { iucn ->
                    ResultBadgeChip(
                        text = BioDexFormatters.iucnShortCode(iucn),
                        color = BioDexColors.TextSecondary,
                    )
                }
            }

            if (response.classification.zoneName != null) {
                Text(
                    text = "Zona verificada: ${response.classification.zoneName}",
                    style = MaterialTheme.typography.bodySmall,
                    color = BioDexColors.Warning,
                )
            }

            Surface(
                color = BioDexColors.SurfaceElevated,
                shape = RoundedCornerShape(14.dp),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    response.taxonomicClass?.let {
                        ResultMetaRow("Clase taxonómica", it)
                    }
                    response.habitat?.let {
                        ResultMetaRow("Hábitat", BioDexFormatters.habitatLabelEs(it))
                    }
                    response.averageMassGrams?.let {
                        ResultMetaRow("Masa promedio", BioDexFormatters.formatMass(it))
                    }
                    ResultMetaRow("Taxon ID", "${ident.taxonId.source}:${ident.taxonId.id}")
                }
            }

            Button(
                onClick = onOpenNaturaLista,
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.SurfaceElevated),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(modifier = Modifier.width(6.dp))
                Text(
                    text = "Ver en NaturaLista ($naturalistaUrl)",
                    color = BioDexColors.Primary,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                )
            }

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Lock,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                    modifier = Modifier.size(14.dp),
                )
                Text(
                    text = "Firma hardware ECDSA P-256 + SHA-256 verificada en servidor",
                    style = MaterialTheme.typography.labelSmall,
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
private fun DiscardedCaptureNotice(
    response: CaptureUploadResponse,
    capturedJpegBytes: ByteArray?,
    naturalistaUrl: String,
    onOpenNaturaLista: () -> Unit,
) {
    val confidencePct = (response.identification.confidence * 100f).roundToInt().coerceIn(0, 100)

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
                text = "Captura Fuera de Rango Natural",
                style = MaterialTheme.typography.headlineSmall,
                color = BioDexColors.Warning,
                fontWeight = FontWeight.Bold,
            )

            val bitmap = remember(capturedJpegBytes) {
                capturedJpegBytes?.let { bytes ->
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
                }
            }
            if (bitmap != null) {
                Image(
                    bitmap = bitmap,
                    contentDescription = response.identification.scientificName,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(170.dp)
                        .clip(RoundedCornerShape(12.dp)),
                )
            }

            Text(
                text = "Detectado: ${response.identification.commonName ?: response.identification.scientificName} (${response.identification.scientificName}) · $confidencePct%",
                style = MaterialTheme.typography.bodyMedium,
                color = BioDexColors.TextPrimary,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = "La coordenada GPS no coincide con un santuario registrado en PostGIS ni con registros naturales verificados en NaturaLista para esta zona.",
                style = MaterialTheme.typography.bodyMedium,
                color = BioDexColors.TextSecondary,
            )
            OutlinedButton(
                onClick = onOpenNaturaLista,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(modifier = Modifier.width(6.dp))
                Text(
                    text = "Consultar distribución en NaturaLista",
                    color = BioDexColors.TextPrimary,
                )
            }
        }
    }
}

@Composable
private fun ResultBadgeChip(text: String, color: Color) {
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
private fun ResultMetaRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = BioDexColors.TextSecondary,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.labelMedium,
            color = BioDexColors.TextPrimary,
            fontWeight = FontWeight.SemiBold,
        )
    }
}
