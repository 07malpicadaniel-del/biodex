package org.biodex.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Explore
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun LoginScreen(
    uiState: BioDexUiState,
    onExplorerNameChange: (String) -> Unit,
    onToggleDevMode: (Boolean) -> Unit,
    onCustomUrlChange: (String) -> Unit,
    onApplyCustomUrl: () -> Unit,
    onRefreshHealth: () -> Unit,
    onLogin: () -> Unit,
) {
    var showServerOptions by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Spacer(modifier = Modifier.height(12.dp))

        // App Header Identity
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(72.dp)
                    .clip(CircleShape)
                    .background(BioDexColors.SurfaceElevated)
                    .border(2.dp, BioDexColors.Primary, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = Icons.Default.Explore,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                    modifier = Modifier.size(36.dp),
                )
            }
            Text(
                text = "BioDex",
                style = MaterialTheme.typography.headlineLarge,
                color = BioDexColors.TextPrimary,
                fontWeight = FontWeight.ExtraBold,
            )
            Text(
                text = "Escáner en vivo y colección de biodiversidad con NaturaLista",
                style = MaterialTheme.typography.bodyMedium,
                color = BioDexColors.TextSecondary,
            )
        }

        // Login Credentials & Explorer Profile Card
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(20.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(18.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    text = "Iniciar Sesión",
                    style = MaterialTheme.typography.titleLarge,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )

                OutlinedTextField(
                    value = uiState.explorerNameInput,
                    onValueChange = onExplorerNameChange,
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    shape = RoundedCornerShape(14.dp),
                    label = { Text("Usuario / ID de Explorador") },
                    leadingIcon = {
                        Icon(
                            imageVector = Icons.Default.Person,
                            contentDescription = null,
                            tint = BioDexColors.Primary,
                        )
                    },
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
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    listOf(
                        BioDexUiState.DEFAULT_EXPLORER_ID,
                        "explorador-veracruz",
                    ).forEach { preset ->
                        FilterChip(
                            selected = uiState.explorerNameInput.trim() == preset,
                            onClick = { onExplorerNameChange(preset) },
                            label = {
                                Text(
                                    text = preset,
                                    maxLines = 1,
                                    softWrap = false,
                                )
                            },
                            colors = FilterChipDefaults.filterChipColors(
                                containerColor = BioDexColors.SurfaceElevated,
                                labelColor = BioDexColors.TextSecondary,
                                selectedContainerColor = BioDexColors.Primary.copy(alpha = 0.22f),
                                selectedLabelColor = BioDexColors.Primary,
                            ),
                        )
                    }
                }
            }
        }

        // Dev Mode Selector Card
        val devAccent = if (uiState.loginDevModeSelected) BioDexColors.Warning else BioDexColors.SurfaceElevated
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(20.dp),
            modifier = Modifier
                .fillMaxWidth()
                .border(
                    width = 1.5.dp,
                    color = devAccent,
                    shape = RoundedCornerShape(20.dp),
                )
                .clickable { onToggleDevMode(!uiState.loginDevModeSelected) },
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
                        .background(
                            if (uiState.loginDevModeSelected) {
                                BioDexColors.Warning.copy(alpha = 0.18f)
                            } else {
                                BioDexColors.SurfaceElevated
                            },
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = Icons.Default.Build,
                        contentDescription = null,
                        tint = if (uiState.loginDevModeSelected) BioDexColors.Warning else BioDexColors.TextSecondary,
                    )
                }

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Modo Dev (Colección Completa)",
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "Desbloquea todas las especies con sus fotos y fichas de NaturaLista para visualizar el almanaque lleno.",
                        style = MaterialTheme.typography.bodySmall,
                        color = BioDexColors.TextSecondary,
                    )
                }

                Switch(
                    checked = uiState.loginDevModeSelected,
                    onCheckedChange = onToggleDevMode,
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = BioDexColors.OnPrimary,
                        checkedTrackColor = BioDexColors.Warning,
                        uncheckedThumbColor = BioDexColors.TextSecondary,
                        uncheckedTrackColor = BioDexColors.SurfaceElevated,
                    ),
                )
            }
        }

        // Enter Button
        Button(
            onClick = onLogin,
            modifier = Modifier
                .fillMaxWidth()
                .height(54.dp),
            shape = RoundedCornerShape(16.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = if (uiState.loginDevModeSelected) BioDexColors.Warning else BioDexColors.Primary,
            ),
        ) {
            Icon(
                imageVector = if (uiState.loginDevModeSelected) Icons.Default.Build else Icons.Default.CameraAlt,
                contentDescription = null,
                tint = BioDexColors.OnPrimary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(modifier = Modifier.size(8.dp))
            Text(
                text = if (uiState.loginDevModeSelected) {
                    "Entrar en Modo Dev"
                } else {
                    "Entrar como Explorador"
                },
                color = BioDexColors.OnPrimary,
                fontWeight = FontWeight.ExtraBold,
                maxLines = 1,
                softWrap = false,
            )
        }

        // Server connection status & optional override
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(16.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Row(
                        modifier = Modifier.weight(1f),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        val dotColor = when (uiState.backendOnline) {
                            true -> BioDexColors.Primary
                            false -> BioDexColors.Danger
                            null -> BioDexColors.Warning
                        }
                        Box(
                            modifier = Modifier
                                .size(9.dp)
                                .clip(CircleShape)
                                .background(dotColor),
                        )
                        Text(
                            text = uiState.backendStatusText,
                            style = MaterialTheme.typography.bodySmall,
                            color = BioDexColors.TextSecondary,
                        )
                    }
                    IconButton(onClick = onRefreshHealth) {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Probar servidor",
                            tint = BioDexColors.Primary,
                        )
                    }
                }

                Text(
                    text = if (showServerOptions) "Ocultar configuración de servidor" else "Configurar URL de servidor (opcional)",
                    style = MaterialTheme.typography.labelMedium,
                    color = BioDexColors.Primary,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.clickable { showServerOptions = !showServerOptions },
                )

                if (showServerOptions) {
                    OutlinedTextField(
                        value = uiState.customServerUrlInput,
                        onValueChange = onCustomUrlChange,
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        shape = RoundedCornerShape(12.dp),
                        label = { Text("URL del backend (Puerto 3030)") },
                        placeholder = { Text("http://127.0.0.1:3030") },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedContainerColor = BioDexColors.SurfaceElevated,
                            unfocusedContainerColor = BioDexColors.SurfaceElevated,
                            focusedBorderColor = BioDexColors.Primary,
                            unfocusedBorderColor = BioDexColors.SurfaceElevated,
                            focusedTextColor = BioDexColors.TextPrimary,
                            unfocusedTextColor = BioDexColors.TextPrimary,
                        ),
                    )
                    OutlinedButton(
                        onClick = onApplyCustomUrl,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Guardar y Probar Servidor", color = Color.White)
                    }
                }
            }
        }
    }
}
