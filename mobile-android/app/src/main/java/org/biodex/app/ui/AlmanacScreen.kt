package org.biodex.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.biodex.app.data.AlmanacItemDto

@Composable
fun AlmanacScreen(
    uiState: BioDexUiState,
    onSearchChange: (String) -> Unit,
    onSelectCategory: (AlmanacCategoryFilter) -> Unit,
    onRefresh: () -> Unit,
    onOpenDetail: (AlmanacItemDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val entries = uiState.filteredAlmanacEntries
    val progress = if (uiState.almanacTotalSpecies > 0) {
        uiState.almanacDiscoveredCount.toFloat() / uiState.almanacTotalSpecies.toFloat()
    } else {
        0f
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Spacer(modifier = Modifier.height(4.dp))

        // Header + Discovery Progress
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Almanaque BioDex",
                            style = MaterialTheme.typography.titleLarge,
                            color = BioDexColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = "${uiState.almanacDiscoveredCount} / ${uiState.almanacTotalSpecies} especies capturadas",
                            style = MaterialTheme.typography.bodySmall,
                            color = BioDexColors.TextSecondary,
                        )
                    }

                    IconButton(onClick = onRefresh) {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Actualizar almanaque",
                            tint = BioDexColors.Primary,
                        )
                    }
                }

                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(7.dp)
                        .clip(RoundedCornerShape(4.dp)),
                    color = BioDexColors.Primary,
                    trackColor = BioDexColors.SurfaceElevated,
                )
            }
        }

        // Search input
        OutlinedTextField(
            value = uiState.almanacSearchQuery,
            onValueChange = onSearchChange,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            shape = RoundedCornerShape(14.dp),
            placeholder = {
                Text(
                    text = "Buscar animal o nombre científico...",
                    color = BioDexColors.TextSecondary,
                )
            },
            leadingIcon = {
                Icon(
                    imageVector = Icons.Default.Search,
                    contentDescription = null,
                    tint = BioDexColors.Primary,
                )
            },
            trailingIcon = {
                if (uiState.almanacSearchQuery.isNotEmpty()) {
                    IconButton(onClick = { onSearchChange("") }) {
                        Icon(
                            imageVector = Icons.Default.Clear,
                            contentDescription = "Limpiar búsqueda",
                            tint = BioDexColors.TextSecondary,
                        )
                    }
                }
            },
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = BioDexColors.Surface,
                unfocusedContainerColor = BioDexColors.Surface,
                focusedBorderColor = BioDexColors.Primary,
                unfocusedBorderColor = BioDexColors.SurfaceElevated,
                focusedTextColor = BioDexColors.TextPrimary,
                unfocusedTextColor = BioDexColors.TextPrimary,
            ),
        )

        // Category filters
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            AlmanacCategoryFilter.entries.forEach { cat ->
                FilterChip(
                    selected = uiState.almanacCategoryFilter == cat,
                    onClick = { onSelectCategory(cat) },
                    label = { Text(cat.label) },
                    colors = FilterChipDefaults.filterChipColors(
                        containerColor = BioDexColors.Surface,
                        labelColor = BioDexColors.TextSecondary,
                        selectedContainerColor = BioDexColors.Primary,
                        selectedLabelColor = BioDexColors.OnPrimary,
                    ),
                )
            }
        }

        when {
            uiState.almanacLoading && uiState.almanacEntries.isEmpty() -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(color = BioDexColors.Primary)
                }
            }

            uiState.almanacError != null && uiState.almanacEntries.isEmpty() -> {
                Surface(
                    color = Color(0xCC7F1D1D),
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = uiState.almanacError,
                        color = Color.White,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }

            entries.isEmpty() -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = "No se encontraron especies con ese filtro.",
                        color = BioDexColors.TextSecondary,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }

            else -> {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    contentPadding = PaddingValues(bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    items(
                        items = entries,
                        key = { "${it.taxonId.source}:${it.taxonId.id}" },
                    ) { item ->
                        AlmanacSpeciesCard(
                            item = item,
                            onClick = { onOpenDetail(item) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AlmanacSpeciesCard(
    item: AlmanacItemDto,
    onClick: () -> Unit,
) {
    val rarityColor = BioDexColors.rarityColor(item.baseCard.rarity)
    val borderColor = if (item.discovered) BioDexColors.Primary else rarityColor.copy(alpha = 0.45f)

    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(1.5.dp, borderColor, RoundedCornerShape(16.dp))
            .clickable(onClick = onClick),
    ) {
        Column(
            modifier = Modifier.padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = item.commonName ?: item.scientificName,
                        style = MaterialTheme.typography.titleMedium,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = item.scientificName,
                        style = MaterialTheme.typography.bodySmall,
                        fontStyle = FontStyle.Italic,
                        color = BioDexColors.TextSecondary,
                    )
                }

                Spacer(modifier = Modifier.width(8.dp))

                Box(
                    modifier = Modifier
                        .size(36.dp)
                        .clip(CircleShape)
                        .background(BioDexColors.SurfaceElevated)
                        .border(1.5.dp, rarityColor, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = item.baseCard.deployCost.toString(),
                        style = MaterialTheme.typography.titleSmall,
                        color = BioDexColors.TextPrimary,
                        fontWeight = FontWeight.ExtraBold,
                    )
                }
            }

            Text(
                text = item.summary,
                style = MaterialTheme.typography.bodySmall,
                color = BioDexColors.TextSecondary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                DiscoveryBadge(
                    discovered = item.discovered,
                    bestVariant = item.bestVariant,
                )
                PillBadge(
                    text = rarityLabelEs(item.baseCard.rarity),
                    color = rarityColor,
                )
                PillBadge(
                    text = habitatLabelEs(item.habitat),
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
fun AlmanacDetailScreen(
    item: AlmanacItemDto,
    onBack: () -> Unit,
) {
    val rarityColor = BioDexColors.rarityColor(item.baseCard.rarity)

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BioDexColors.Background)
            .statusBarsPadding()
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = "Volver al Almanaque",
                    tint = BioDexColors.Primary,
                )
            }
            Text(
                text = "Ficha del Almanaque",
                style = MaterialTheme.typography.titleMedium,
                color = BioDexColors.TextPrimary,
                fontWeight = FontWeight.Bold,
            )
        }

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(22.dp),
            modifier = Modifier
                .fillMaxWidth()
                .border(2.dp, rarityColor, RoundedCornerShape(22.dp)),
        ) {
            Column(
                modifier = Modifier.padding(18.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = item.commonName ?: item.scientificName,
                            style = MaterialTheme.typography.headlineSmall,
                            color = BioDexColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = item.scientificName,
                            style = MaterialTheme.typography.bodyMedium,
                            fontStyle = FontStyle.Italic,
                            color = BioDexColors.TextSecondary,
                        )
                    }

                    Box(
                        modifier = Modifier
                            .size(46.dp)
                            .clip(CircleShape)
                            .background(BioDexColors.Primary),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = item.baseCard.deployCost.toString(),
                            style = MaterialTheme.typography.titleLarge,
                            color = BioDexColors.OnPrimary,
                            fontWeight = FontWeight.ExtraBold,
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    DiscoveryBadge(
                        discovered = item.discovered,
                        bestVariant = item.bestVariant,
                    )
                    PillBadge(
                        text = "${rarityLabelEs(item.baseCard.rarity)} · UICN ${item.iucnStatus}",
                        color = rarityColor,
                    )
                }

                Text(
                    text = item.summary,
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextPrimary,
                )

                item.regionHint?.let { region ->
                    Text(
                        text = "Dónde encontrarlo: $region",
                        style = MaterialTheme.typography.bodySmall,
                        color = BioDexColors.Primary,
                    )
                }

                // Biological profile grid
                Surface(
                    color = BioDexColors.SurfaceElevated,
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier.padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        BioMetaRow("Taxonomía", "${item.phylum} · ${item.taxonomicClass}")
                        BioMetaRow("Masa promedio", formatMass(item.averageMassGrams))
                        BioMetaRow("Dieta", dietLabelEs(item.diet))
                        BioMetaRow("Hábitat", habitatLabelEs(item.habitat))
                        val defensesText = if (item.defenses.isEmpty()) {
                            "Ninguna especial"
                        } else {
                            item.defenses.joinToString(", ", transform = ::defenseLabelEs)
                        }
                        BioMetaRow("Defensas naturales", defensesText)
                    }
                }

                // Base procedural stats (Wild 100%)
                Text(
                    text = "Estadísticas Base (Salvaje 100%)",
                    style = MaterialTheme.typography.titleSmall,
                    color = BioDexColors.TextSecondary,
                    fontWeight = FontWeight.SemiBold,
                )
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(12.dp))
                        .background(BioDexColors.SurfaceElevated)
                        .padding(vertical = 12.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    AlmanacStatCell("HP", item.baseCard.stats.hp)
                    AlmanacStatCell("ATK", item.baseCard.stats.attack)
                    AlmanacStatCell("DEF", item.baseCard.stats.defense)
                    AlmanacStatCell("SPD", item.baseCard.stats.speed)
                }

                // Moves
                Text(
                    text = "Movimientos Base",
                    style = MaterialTheme.typography.titleSmall,
                    color = BioDexColors.TextSecondary,
                    fontWeight = FontWeight.SemiBold,
                )
                item.baseCard.moves.forEach { move ->
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

                Text(
                    text = "Taxon ID: ${item.taxonId.source}:${item.taxonId.id} · Capturas registradas: ${item.captureCount}",
                    style = MaterialTheme.typography.labelSmall,
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
private fun DiscoveryBadge(discovered: Boolean, bestVariant: String?) {
    val color = if (discovered) BioDexColors.Primary else BioDexColors.TextSecondary
    val label = if (discovered) {
        when (bestVariant) {
            "wild" -> "CAPTURADO · SALVAJE"
            "sanctuary" -> "CAPTURADO · SANTUARIO"
            else -> "CAPTURADO"
        }
    } else {
        "POR DESCUBRIR"
    }

    Surface(
        color = color.copy(alpha = 0.16f),
        shape = RoundedCornerShape(8.dp),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (discovered) {
                Icon(
                    imageVector = Icons.Default.CheckCircle,
                    contentDescription = null,
                    tint = color,
                    modifier = Modifier.size(13.dp),
                )
            }
            Text(
                text = label,
                style = MaterialTheme.typography.labelSmall,
                color = color,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

@Composable
private fun PillBadge(text: String, color: Color) {
    Surface(
        color = color.copy(alpha = 0.15f),
        shape = RoundedCornerShape(8.dp),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelSmall,
            color = color,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

@Composable
private fun BioMetaRow(label: String, value: String) {
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

@Composable
private fun AlmanacStatCell(label: String, value: Int) {
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

private fun formatMass(grams: Long): String = when {
    grams >= 1_000_000L -> "%.1f t".format(grams / 1_000_000.0)
    grams >= 1_000L -> "%.1f kg".format(grams / 1_000.0)
    else -> "$grams g"
}

private fun rarityLabelEs(rarity: String): String = when (rarity.lowercase()) {
    "uncommon" -> "Poco Común"
    "rare" -> "Rara"
    "epic" -> "Épica"
    "mythic" -> "Mítica"
    else -> "Común"
}

private fun habitatLabelEs(habitat: String): String = when (habitat.lowercase()) {
    "aerial" -> "Aéreo"
    "terrestrial" -> "Terrestre"
    "freshwater" -> "Dulceacuícola"
    "marine" -> "Marino"
    "abyssal" -> "Abisal"
    else -> habitat
}

private fun dietLabelEs(diet: String): String = when (diet.lowercase()) {
    "carnivore" -> "Carnívoro"
    "herbivore" -> "Herbívoro"
    "omnivore" -> "Omnívoro"
    "detritivore" -> "Detritívoro"
    else -> diet
}

private fun defenseLabelEs(defense: String): String = when (defense.lowercase()) {
    "venom" -> "Veneno"
    "camouflage" -> "Camuflaje"
    "shell_armor" -> "Caparazón"
    "spines" -> "Espinas"
    "ink" -> "Tinta"
    "speed" -> "Velocidad"
    else -> defense
}
