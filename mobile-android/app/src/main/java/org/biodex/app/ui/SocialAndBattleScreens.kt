package org.biodex.app.ui

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun BattleScreen(
    uiState: BioDexUiState,
    onGoToScanner: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val capturedCards = uiState.almanacEntries.filter { it.discovered }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        SectionHeader(
            title = "Arena de Combate",
            subtitle = "Batallas deterministas por turnos (3v3 con energía)",
            icon = Icons.Default.Bolt,
            accent = BioDexColors.Warning,
        )

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier
                .fillMaxWidth()
                .border(1.5.dp, BioDexColors.Warning.copy(alpha = 0.5f), RoundedCornerShape(18.dp)),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Tu Escuadrón Disponible",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    StatusPill(text = "FASE 2", color = BioDexColors.Warning)
                }

                Text(
                    text = "${capturedCards.size} de 3 criaturas mínimas capturadas para formar un equipo competitivo.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextSecondary,
                )

                if (capturedCards.isNotEmpty()) {
                    capturedCards.take(3).forEach { entry ->
                        Surface(
                            color = BioDexColors.SurfaceElevated,
                            shape = RoundedCornerShape(12.dp),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Row(
                                modifier = Modifier.padding(12.dp),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = entry.commonName ?: entry.scientificName,
                                        style = MaterialTheme.typography.titleSmall,
                                        color = BioDexColors.TextPrimary,
                                        fontWeight = FontWeight.Bold,
                                    )
                                    Text(
                                        text = "${entry.baseCard.stats.hp} HP · ${entry.baseCard.stats.attack} ATK",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = BioDexColors.TextSecondary,
                                    )
                                }
                                StatusPill(
                                    text = "Coste ${entry.baseCard.deployCost}",
                                    color = BioDexColors.Primary,
                                )
                            }
                        }
                    }
                } else {
                    Button(
                        onClick = onGoToScanner,
                        modifier = Modifier.fillMaxWidth(),
                        colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                    ) {
                        Text(
                            text = "Capturar Criaturas en el Escáner",
                            color = BioDexColors.OnPrimary,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                }
            }
        }

        InfoRuleCard(
            title = "Reglas del Motor Determinista",
            bullets = listOf(
                "Combates 1v1 con equipos de 3 criaturas y gestión de energía (coste 1–10).",
                "Las capturas Salvajes conservan el 100% del potencial genético; Santuario aplica 85%.",
                "Misma secuencia de jugadas + semilla produce resultado verificable en servidor.",
            ),
        )
    }
}

@Composable
fun FriendsScreen(
    uiState: BioDexUiState,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        SectionHeader(
            title = "Amigos e Intercambios",
            subtitle = "Red de exploradores e intercambio P2P verificable",
            icon = Icons.Default.Group,
            accent = BioDexColors.Primary,
        )

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Tu Código de Explorador",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    StatusPill(text = "ACTIVO", color = BioDexColors.Primary)
                }

                Surface(
                    color = BioDexColors.SurfaceElevated,
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = uiState.playerId,
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.Primary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }
        }

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.SwapHoriz,
                        contentDescription = null,
                        tint = BioDexColors.Warning,
                    )
                    Text(
                        text = "Mercado P2P y Lista de Amigos",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                }
                Text(
                    text = "En Fase 2 podrás agregar exploradores cercanos e intercambiar cartas conservando su procedencia original (coordenadas, fecha y variante Salvaje/Santuario).",
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextSecondary,
                )
                StatusPill(text = "FASE 2 · SINCRONIZACIÓN SOCIAL", color = BioDexColors.Warning)
            }
        }
    }
}

@Composable
fun ProfileScreen(
    uiState: BioDexUiState,
    onCustomUrlChange: (String) -> Unit,
    onApplyCustomUrl: () -> Unit,
    onResetDefaultUrl: () -> Unit,
    onRefreshConnection: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        SectionHeader(
            title = "Perfil del Explorador",
            subtitle = "Identidad hardware y diagnóstico de conexión",
            icon = Icons.Default.Person,
            accent = BioDexColors.Primary,
        )

        // Explorer stats summary
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .size(48.dp)
                            .clip(CircleShape)
                            .background(BioDexColors.SurfaceElevated)
                            .border(1.5.dp, BioDexColors.Primary, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Default.Person,
                            contentDescription = null,
                            tint = BioDexColors.Primary,
                        )
                    }
                    Column {
                        Text(
                            text = uiState.playerId,
                            style = MaterialTheme.typography.titleMedium,
                            color = BioDexColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                        )
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Default.Lock,
                                contentDescription = null,
                                tint = BioDexColors.Primary,
                                modifier = Modifier.size(14.dp),
                            )
                            Text(
                                text = "Android Keystore ECDSA P-256 activo",
                                style = MaterialTheme.typography.labelSmall,
                                color = BioDexColors.Primary,
                            )
                        }
                    }
                }

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(12.dp))
                        .background(BioDexColors.SurfaceElevated)
                        .padding(vertical = 12.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    ProfileStat(
                        label = "Especies",
                        value = "${uiState.almanacDiscoveredCount}/${uiState.almanacTotalSpecies}",
                    )
                    ProfileStat(
                        label = "Capturas",
                        value = uiState.totalCapturesCount.toString(),
                    )
                    ProfileStat(
                        label = "IMU",
                        value = if (uiState.telemetry.hasImuFix) "OK" else "...",
                    )
                }
            }
        }

        // Connection / Port diagnostics & URL editor
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Conexión al Servidor (Puerto 3000)",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.weight(1f),
                    )
                    val online = uiState.backendOnline == true
                    StatusPill(
                        text = if (online) "EN LÍNEA" else "SIN CONEXIÓN",
                        color = if (online) BioDexColors.Primary else BioDexColors.Danger,
                    )
                }

                Text(
                    text = "Endpoint activo: ${uiState.activeBackendUrl}",
                    style = MaterialTheme.typography.bodySmall,
                    color = BioDexColors.TextPrimary,
                )
                Text(
                    text = uiState.backendStatusText,
                    style = MaterialTheme.typography.labelSmall,
                    color = BioDexColors.TextSecondary,
                )

                OutlinedTextField(
                    value = uiState.customServerUrlInput,
                    onValueChange = onCustomUrlChange,
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    label = { Text("URL personalizada (opcional)") },
                    placeholder = { Text("http://192.168.1.100:3000") },
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedContainerColor = BioDexColors.SurfaceElevated,
                        unfocusedContainerColor = BioDexColors.SurfaceElevated,
                        focusedBorderColor = BioDexColors.Primary,
                        unfocusedBorderColor = BioDexColors.SurfaceElevated,
                        focusedTextColor = BioDexColors.TextPrimary,
                        unfocusedTextColor = BioDexColors.TextPrimary,
                    ),
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(
                        onClick = onApplyCustomUrl,
                        modifier = Modifier.weight(1f),
                        colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                    ) {
                        Icon(
                            imageVector = Icons.Default.CheckCircle,
                            contentDescription = null,
                            tint = BioDexColors.OnPrimary,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.size(6.dp))
                        Text(
                            text = "Guardar y Probar",
                            color = BioDexColors.OnPrimary,
                            fontWeight = FontWeight.Bold,
                        )
                    }

                    OutlinedButton(
                        onClick = {
                            onResetDefaultUrl()
                            onRefreshConnection()
                        },
                    ) {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Auto-detectar",
                            tint = BioDexColors.Primary,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SectionHeader(
    title: String,
    subtitle: String,
    icon: ImageVector,
    accent: Color,
) {
    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(18.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .clip(CircleShape)
                    .background(accent.copy(alpha = 0.16f)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = accent,
                )
            }
            Column {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleLarge,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
private fun InfoRuleCard(
    title: String,
    bullets: List<String>,
) {
    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(18.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                color = BioDexColors.TextPrimary,
                fontWeight = FontWeight.Bold,
            )
            bullets.forEach { line ->
                Text(
                    text = "• $line",
                    style = MaterialTheme.typography.bodySmall,
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
private fun StatusPill(text: String, color: Color) {
    Surface(
        color = color.copy(alpha = 0.16f),
        shape = RoundedCornerShape(8.dp),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelSmall,
            color = color,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            softWrap = false,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

@Composable
private fun ProfileStat(label: String, value: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = BioDexColors.TextSecondary,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.titleLarge,
            color = BioDexColors.TextPrimary,
            fontWeight = FontWeight.Bold,
        )
    }
}
