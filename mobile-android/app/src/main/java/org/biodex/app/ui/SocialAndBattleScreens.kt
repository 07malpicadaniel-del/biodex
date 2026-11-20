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
import androidx.compose.material.icons.automirrored.filled.ExitToApp
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Build
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
    onGoToScanner: () -> Unit,
    onGoToCollection: () -> Unit,
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
            title = "Arena de Combate",
            subtitle = "Disponible en Fase 2",
            icon = Icons.Default.Bolt,
            accent = BioDexColors.Warning,
        )

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier
                .fillMaxWidth()
                .border(1.5.dp, BioDexColors.Warning.copy(alpha = 0.45f), RoundedCornerShape(18.dp)),
        ) {
            Column(
                modifier = Modifier.padding(18.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                StatusPill(text = "FASE 2 · EN PREPARACIÓN", color = BioDexColors.Warning)
                Text(
                    text = "El sistema de combates aún está desactivado mientras completamos el Escáner en vivo y tu Colección enciclopédica con NaturaLista.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextSecondary,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(
                        onClick = onGoToScanner,
                        modifier = Modifier.weight(1f),
                        colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                    ) {
                        Text("Ir al Escáner", color = BioDexColors.OnPrimary, fontWeight = FontWeight.Bold)
                    }
                    OutlinedButton(
                        onClick = onGoToCollection,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text("Ver Colección", color = BioDexColors.TextPrimary)
                    }
                }
            }
        }
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
            subtitle = "Red de exploradores (Fase 2)",
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
                        text = "Tu ID de Explorador",
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
                        text = "Intercambios P2P y Amigos",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                }
                Text(
                    text = "Disponible en la Fase 2 para agregar exploradores e intercambiar registros conservando su ubicación y procedencia verificada.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextSecondary,
                )
                StatusPill(text = "FASE 2 · EN PREPARACIÓN", color = BioDexColors.Warning)
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
    onLogout: () -> Unit,
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
            subtitle = "Sesión activa y diagnóstico de conexión",
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
                            .border(
                                1.5.dp,
                                if (uiState.isDevMode) BioDexColors.Warning else BioDexColors.Primary,
                                CircleShape,
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = if (uiState.isDevMode) Icons.Default.Build else Icons.Default.Person,
                            contentDescription = null,
                            tint = if (uiState.isDevMode) BioDexColors.Warning else BioDexColors.Primary,
                        )
                    }
                    Column(modifier = Modifier.weight(1f)) {
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
                                text = "Keystore ECDSA P-256 activo",
                                style = MaterialTheme.typography.labelSmall,
                                color = BioDexColors.Primary,
                            )
                        }
                    }

                    StatusPill(
                        text = if (uiState.isDevMode) "MODO DEV" else "EXPLORADOR",
                        color = if (uiState.isDevMode) BioDexColors.Warning else BioDexColors.Primary,
                    )
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
                        label = "Registros",
                        value = uiState.totalCapturesCount.toString(),
                    )
                    ProfileStat(
                        label = "IMU",
                        value = if (uiState.telemetry.hasImuFix) "OK" else "...",
                    )
                }

                OutlinedButton(
                    onClick = onLogout,
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ExitToApp,
                        contentDescription = null,
                        tint = BioDexColors.Warning,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(modifier = Modifier.size(8.dp))
                    Text(
                        text = "Cerrar Sesión / Cambiar Modo Dev",
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.SemiBold,
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
                        text = "Conexión al Servidor (Puerto 3030)",
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
                    placeholder = { Text("http://192.168.1.100:3030") },
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
