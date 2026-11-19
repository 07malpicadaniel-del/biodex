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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Explore
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Pets
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil.compose.SubcomposeAsyncImage
import org.biodex.app.data.AlmanacItemDto
import org.biodex.app.data.NaturaListaSpeciesDto

@Composable
fun AlmanacScreen(
    uiState: BioDexUiState,
    onSelectSubTab: (CollectionSubTab) -> Unit,
    onSearchChange: (String) -> Unit,
    onSelectCategory: (AlmanacCategoryFilter) -> Unit,
    onSelectCaptureFilter: (AlmanacCaptureFilter) -> Unit,
    onRefresh: () -> Unit,
    onOpenDetail: (AlmanacItemDto) -> Unit,
    onNaturaListaQueryChange: (String) -> Unit,
    onSubmitNaturaListaSearch: () -> Unit,
    onRefreshNearbyNaturaLista: () -> Unit,
    onInspectNaturaListaSpecies: (NaturaListaSpeciesDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val progress = if (uiState.almanacTotalSpecies > 0) {
        uiState.almanacDiscoveredCount.toFloat() / uiState.almanacTotalSpecies.toFloat()
    } else {
        0f
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Spacer(modifier = Modifier.height(4.dp))

            // Collection Header + Dev Mode Banner + Progress
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
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    text = "Colección",
                                    style = MaterialTheme.typography.titleLarge,
                                    color = BioDexColors.TextPrimary,
                                    fontWeight = FontWeight.Bold,
                                )
                                if (uiState.isDevMode) {
                                    Surface(
                                        color = BioDexColors.Warning.copy(alpha = 0.2f),
                                        shape = RoundedCornerShape(8.dp),
                                    ) {
                                        Row(
                                            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                                            verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                                        ) {
                                            Icon(
                                                imageVector = Icons.Default.Build,
                                                contentDescription = null,
                                                tint = BioDexColors.Warning,
                                                modifier = Modifier.size(12.dp),
                                            )
                                            Text(
                                                text = "MODO DEV",
                                                style = MaterialTheme.typography.labelSmall,
                                                color = BioDexColors.Warning,
                                                fontWeight = FontWeight.ExtraBold,
                                            )
                                        }
                                    }
                                }
                            }
                            Text(
                                text = "${uiState.almanacDiscoveredCount} / ${uiState.almanacTotalSpecies} especies desbloqueadas",
                                style = MaterialTheme.typography.bodySmall,
                                color = BioDexColors.TextSecondary,
                            )
                        }

                        Row(verticalAlignment = Alignment.CenterVertically) {
                            IconButton(
                                onClick = {
                                    BioDexFormatters.openExternalUrl(
                                        context,
                                        "https://www.naturalista.mx/taxa",
                                    )
                                },
                            ) {
                                Icon(
                                    imageVector = Icons.Default.Public,
                                    contentDescription = "Abrir portal NaturaLista",
                                    tint = BioDexColors.Primary,
                                )
                            }
                            IconButton(
                                onClick = {
                                    when (uiState.collectionSubTab) {
                                        CollectionSubTab.Almanac -> onRefresh()
                                        CollectionSubTab.Nearby -> onRefreshNearbyNaturaLista()
                                        CollectionSubTab.Search -> onSubmitNaturaListaSearch()
                                    }
                                },
                            ) {
                                Icon(
                                    imageVector = Icons.Default.Refresh,
                                    contentDescription = "Actualizar",
                                    tint = BioDexColors.Primary,
                                )
                            }
                        }
                    }

                    LinearProgressIndicator(
                        progress = { progress },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(7.dp)
                            .clip(RoundedCornerShape(4.dp)),
                        color = if (uiState.isDevMode) BioDexColors.Warning else BioDexColors.Primary,
                        trackColor = BioDexColors.SurfaceElevated,
                    )

                    // 3 Collection Sub-Tabs (Colección / Cerca / NaturaLista)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(BioDexColors.SurfaceElevated)
                            .padding(4.dp),
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        CollectionSubTab.entries.forEach { tab ->
                            val selected = uiState.collectionSubTab == tab
                            val shortLabel = when (tab) {
                                CollectionSubTab.Almanac -> "Colección"
                                CollectionSubTab.Nearby -> "Cerca"
                                CollectionSubTab.Search -> "NaturaLista"
                            }
                            Surface(
                                color = if (selected) BioDexColors.Primary else Color.Transparent,
                                shape = RoundedCornerShape(9.dp),
                                modifier = Modifier
                                    .weight(1f)
                                    .clickable { onSelectSubTab(tab) },
                            ) {
                                Box(
                                    modifier = Modifier.padding(vertical = 8.dp, horizontal = 6.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = shortLabel,
                                        style = MaterialTheme.typography.labelLarge,
                                        color = if (selected) BioDexColors.OnPrimary else BioDexColors.TextSecondary,
                                        fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                        maxLines = 1,
                                        softWrap = false,
                                    )
                                }
                            }
                        }
                    }
                }
            }

            when (uiState.collectionSubTab) {
                CollectionSubTab.Almanac -> {
                    AlmanacCollectionBody(
                        uiState = uiState,
                        onSearchChange = onSearchChange,
                        onSelectCategory = onSelectCategory,
                        onSelectCaptureFilter = onSelectCaptureFilter,
                        onOpenDetail = onOpenDetail,
                        modifier = Modifier.weight(1f),
                    )
                }

                CollectionSubTab.Nearby -> {
                    NaturaListaNearbyBody(
                        uiState = uiState,
                        onRefresh = onRefreshNearbyNaturaLista,
                        onInspectSpecies = onInspectNaturaListaSpecies,
                        modifier = Modifier.weight(1f),
                    )
                }

                CollectionSubTab.Search -> {
                    NaturaListaSearchBody(
                        uiState = uiState,
                        onQueryChange = onNaturaListaQueryChange,
                        onSubmitSearch = onSubmitNaturaListaSearch,
                        onInspectSpecies = onInspectNaturaListaSpecies,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
        }

        if (uiState.inspectingTaxonLoading) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color(0x9908120E)),
                contentAlignment = Alignment.Center,
            ) {
                Surface(
                    color = BioDexColors.Surface,
                    shape = RoundedCornerShape(16.dp),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        CircularProgressIndicator(
                            color = BioDexColors.Primary,
                            modifier = Modifier.size(24.dp),
                            strokeWidth = 2.5.dp,
                        )
                        Text(
                            text = "Cargando ficha de NaturaLista...",
                            color = BioDexColors.TextPrimary,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AlmanacCollectionBody(
    uiState: BioDexUiState,
    onSearchChange: (String) -> Unit,
    onSelectCategory: (AlmanacCategoryFilter) -> Unit,
    onSelectCaptureFilter: (AlmanacCaptureFilter) -> Unit,
    onOpenDetail: (AlmanacItemDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val entries = uiState.filteredAlmanacEntries

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        OutlinedTextField(
            value = uiState.almanacSearchQuery,
            onValueChange = onSearchChange,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            shape = RoundedCornerShape(14.dp),
            placeholder = {
                Text(
                    text = "Filtrar colección por nombre común o científico...",
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

        // Discovery state filter row
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            AlmanacCaptureFilter.entries.forEach { filter ->
                FilterChip(
                    selected = uiState.almanacCaptureFilter == filter,
                    onClick = { onSelectCaptureFilter(filter) },
                    label = {
                        val suffix = if (filter == AlmanacCaptureFilter.Captured) {
                            " (${uiState.almanacDiscoveredCount})"
                        } else {
                            ""
                        }
                        Text("${filter.label}$suffix")
                    },
                    colors = FilterChipDefaults.filterChipColors(
                        containerColor = BioDexColors.Surface,
                        labelColor = BioDexColors.TextSecondary,
                        selectedContainerColor = BioDexColors.Warning,
                        selectedLabelColor = BioDexColors.OnPrimary,
                    ),
                )
            }
        }

        // Taxonomic category filter row
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
                CenteredLoader()
            }

            uiState.almanacError != null && uiState.almanacEntries.isEmpty() -> {
                ErrorBanner(uiState.almanacError)
            }

            entries.isEmpty() -> {
                CenteredEmptyText("No se encontraron especies con ese filtro.")
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
private fun NaturaListaNearbyBody(
    uiState: BioDexUiState,
    onRefresh: () -> Unit,
    onInspectSpecies: (NaturaListaSpeciesDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val results = uiState.naturalistaNearbyResults

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(14.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Row(
                    modifier = Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.LocationOn,
                        contentDescription = null,
                        tint = BioDexColors.Primary,
                        modifier = Modifier.size(18.dp),
                    )
                    val lat = uiState.telemetry.latitude
                    val lon = uiState.telemetry.longitude
                    Text(
                        text = if (results != null) {
                            results.querySummary
                        } else if (lat != null && lon != null) {
                            "Fauna reportada en NaturaLista cerca de %.3f, %.3f".format(lat, lon)
                        } else {
                            "Esperando coordenadas GPS para consultar NaturaLista..."
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = BioDexColors.TextPrimary,
                    )
                }

                if (results != null) {
                    IconButton(
                        onClick = { BioDexFormatters.openExternalUrl(context, results.sourceUrl) },
                    ) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                            contentDescription = "Abrir mapa en NaturaLista",
                            tint = BioDexColors.Primary,
                        )
                    }
                }
            }
        }

        when {
            uiState.naturalistaNearbyLoading -> CenteredLoader()
            uiState.naturalistaNearbyError != null -> {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    ErrorBanner(uiState.naturalistaNearbyError)
                    Button(
                        onClick = onRefresh,
                        colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                    ) {
                        Text("Reintentar con GPS actual", color = BioDexColors.OnPrimary)
                    }
                }
            }

            results == null || results.entries.isEmpty() -> {
                CenteredEmptyText("Pulsa Actualizar para cargar especies cercanas desde NaturaLista.")
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
                        items = results.entries,
                        key = { "${it.taxonId.source}:${it.taxonId.id}" },
                    ) { species ->
                        NaturaListaSpeciesRow(
                            species = species,
                            onInspect = { onInspectSpecies(species) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun NaturaListaSearchBody(
    uiState: BioDexUiState,
    onQueryChange: (String) -> Unit,
    onSubmitSearch: () -> Unit,
    onInspectSpecies: (NaturaListaSpeciesDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val results = uiState.naturalistaSearchResults

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = uiState.naturalistaSearchQuery,
                onValueChange = onQueryChange,
                modifier = Modifier.weight(1f),
                singleLine = true,
                shape = RoundedCornerShape(14.dp),
                placeholder = {
                    Text(
                        text = "Buscar animal en NaturaLista (ej. tucán, jaguar)...",
                        color = BioDexColors.TextSecondary,
                    )
                },
                leadingIcon = {
                    Icon(
                        imageVector = Icons.Default.Explore,
                        contentDescription = null,
                        tint = BioDexColors.Primary,
                    )
                },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { onSubmitSearch() }),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedContainerColor = BioDexColors.Surface,
                    unfocusedContainerColor = BioDexColors.Surface,
                    focusedBorderColor = BioDexColors.Primary,
                    unfocusedBorderColor = BioDexColors.SurfaceElevated,
                    focusedTextColor = BioDexColors.TextPrimary,
                    unfocusedTextColor = BioDexColors.TextPrimary,
                ),
            )
            Button(
                onClick = onSubmitSearch,
                modifier = Modifier.height(54.dp),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
            ) {
                Icon(
                    imageVector = Icons.Default.Search,
                    contentDescription = "Buscar en NaturaLista",
                    tint = BioDexColors.OnPrimary,
                )
            }
        }

        if (results != null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = results.querySummary,
                    style = MaterialTheme.typography.labelMedium,
                    color = BioDexColors.TextSecondary,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = "Abrir en web",
                    style = MaterialTheme.typography.labelMedium,
                    color = BioDexColors.Primary,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.clickable {
                        BioDexFormatters.openExternalUrl(context, results.sourceUrl)
                    },
                )
            }
        }

        when {
            uiState.naturalistaSearchLoading -> CenteredLoader()
            uiState.naturalistaSearchError != null -> ErrorBanner(uiState.naturalistaSearchError)
            results == null -> {
                CenteredEmptyText(
                    "Consulta cualquier especie animal en el catálogo oficial de NaturaLista / iNaturalist y abre su ficha o URL.",
                )
            }

            results.entries.isEmpty() -> {
                CenteredEmptyText("No se encontraron especies animales con esa búsqueda en NaturaLista.")
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
                        items = results.entries,
                        key = { "${it.taxonId.source}:${it.taxonId.id}" },
                    ) { species ->
                        NaturaListaSpeciesRow(
                            species = species,
                            onInspect = { onInspectSpecies(species) },
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
    val context = LocalContext.current
    val accentColor = iucnAccentColor(item.iucnStatus)
    val borderColor = if (item.discovered) BioDexColors.Primary else BioDexColors.SurfaceElevated
    val naturalistaUrl = BioDexFormatters.naturalistaUrl(item.taxonId, item.naturalistaUrl)

    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(1.5.dp, borderColor, RoundedCornerShape(16.dp))
            .clickable(onClick = onClick),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SpeciesThumbnail(
                imageUrl = item.imageUrl,
                contentDescription = item.commonName ?: item.scientificName,
                unlocked = item.discovered,
                sizeDp = 76,
            )

            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.Top,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = item.commonName ?: item.scientificName,
                            style = MaterialTheme.typography.titleMedium,
                            color = if (item.discovered) BioDexColors.TextPrimary else BioDexColors.TextSecondary,
                            fontWeight = FontWeight.Bold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = item.scientificName,
                            style = MaterialTheme.typography.bodySmall,
                            fontStyle = FontStyle.Italic,
                            color = BioDexColors.TextSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }

                    IconButton(
                        onClick = { BioDexFormatters.openExternalUrl(context, naturalistaUrl) },
                        modifier = Modifier.size(30.dp),
                    ) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                            contentDescription = "Abrir en NaturaLista",
                            tint = BioDexColors.Primary,
                            modifier = Modifier.size(18.dp),
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
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    DiscoveryBadge(
                        discovered = item.discovered,
                        bestVariant = item.bestVariant,
                        compact = true,
                    )
                    PillBadge(
                        text = BioDexFormatters.iucnShortCode(item.iucnStatus),
                        color = accentColor,
                    )
                    PillBadge(
                        text = BioDexFormatters.habitatLabelEs(item.habitat),
                        color = BioDexColors.TextSecondary,
                    )
                }
            }
        }
    }
}

@Composable
private fun NaturaListaSpeciesRow(
    species: NaturaListaSpeciesDto,
    onInspect: () -> Unit,
) {
    val context = LocalContext.current

    Surface(
        color = BioDexColors.Surface,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier
            .fillMaxWidth()
            .border(
                1.dp,
                if (species.discovered) BioDexColors.Primary else BioDexColors.SurfaceElevated,
                RoundedCornerShape(16.dp),
            )
            .clickable(onClick = onInspect),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SpeciesThumbnail(
                imageUrl = species.imageUrl,
                contentDescription = species.commonName ?: species.scientificName,
                unlocked = true,
                sizeDp = 72,
            )

            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                Text(
                    text = species.commonName ?: species.scientificName,
                    style = MaterialTheme.typography.titleMedium,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = species.scientificName,
                    style = MaterialTheme.typography.bodySmall,
                    fontStyle = FontStyle.Italic,
                    color = BioDexColors.TextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (species.discovered) {
                        DiscoveryBadge(
                            discovered = true,
                            bestVariant = species.bestVariant,
                            compact = true,
                        )
                    }
                    species.localObservationsCount?.let { count ->
                        PillBadge(
                            text = "${BioDexFormatters.formatObservations(count)} en tu zona",
                            color = BioDexColors.Primary,
                        )
                    } ?: species.globalObservationsCount?.let { count ->
                        PillBadge(
                            text = "${BioDexFormatters.formatObservations(count)} obs.",
                            color = BioDexColors.TextSecondary,
                        )
                    }
                    if (species.iucnStatus != "UNKNOWN") {
                        PillBadge(
                            text = BioDexFormatters.iucnShortCode(species.iucnStatus),
                            color = iucnAccentColor(species.iucnStatus),
                        )
                    }
                }
            }

            IconButton(
                onClick = { BioDexFormatters.openExternalUrl(context, species.naturalistaUrl) },
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                    contentDescription = "Abrir URL de NaturaLista",
                    tint = BioDexColors.Primary,
                )
            }
        }
    }
}

@Composable
fun AlmanacDetailScreen(
    item: AlmanacItemDto,
    isDevMode: Boolean,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val accentColor = iucnAccentColor(item.iucnStatus)
    val naturalistaUrl = BioDexFormatters.naturalistaUrl(item.taxonId, item.naturalistaUrl)

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
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onBack) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Volver a Colección",
                        tint = BioDexColors.Primary,
                    )
                }
                Text(
                    text = "Ficha de Colección",
                    style = MaterialTheme.typography.titleMedium,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )
            }

            if (isDevMode) {
                PillBadge(text = "MODO DEV", color = BioDexColors.Warning)
            }
        }

        Surface(
            color = BioDexColors.Surface,
            shape = RoundedCornerShape(22.dp),
            modifier = Modifier
                .fillMaxWidth()
                .border(2.dp, if (item.discovered) BioDexColors.Primary else accentColor, RoundedCornerShape(22.dp)),
        ) {
            Column(
                modifier = Modifier.padding(18.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // Hero Wildlife Photograph from NaturaLista
                SpeciesHeroImage(
                    imageUrl = item.imageUrl,
                    contentDescription = item.commonName ?: item.scientificName,
                    unlocked = item.discovered,
                    attribution = item.photoAttribution,
                )

                Column {
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

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    DiscoveryBadge(
                        discovered = item.discovered,
                        bestVariant = item.bestVariant,
                    )
                    PillBadge(
                        text = BioDexFormatters.iucnLabelEs(item.iucnStatus),
                        color = accentColor,
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

                // Biological profile grid (NO combat info)
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
                        BioMetaRow("Masa promedio", BioDexFormatters.formatMass(item.averageMassGrams))
                        BioMetaRow("Dieta", BioDexFormatters.dietLabelEs(item.diet))
                        BioMetaRow("Hábitat", BioDexFormatters.habitatLabelEs(item.habitat))
                        val defensesText = if (item.defenses.isEmpty()) {
                            "Ninguna especial"
                        } else {
                            item.defenses.joinToString(", ", transform = BioDexFormatters::defenseLabelEs)
                        }
                        BioMetaRow("Defensas naturales", defensesText)
                        item.observationsCount?.let { count ->
                            BioMetaRow(
                                "Registros NaturaLista",
                                BioDexFormatters.formatObservations(count),
                            )
                        }
                        item.lastZoneName?.let { zone ->
                            BioMetaRow("Última zona verificada", zone)
                        }
                        BioMetaRow(
                            "Capturas registradas",
                            item.captureCount.toString(),
                        )
                    }
                }

                // NaturaLista URL Actions
                Button(
                    onClick = { BioDexFormatters.openExternalUrl(context, naturalistaUrl) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(48.dp),
                    shape = RoundedCornerShape(12.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                        contentDescription = null,
                        tint = BioDexColors.OnPrimary,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Abrir ficha en NaturaLista",
                        color = BioDexColors.OnPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (!item.wikipediaUrl.isNullOrBlank()) {
                        OutlinedButton(
                            onClick = {
                                BioDexFormatters.openExternalUrl(context, item.wikipediaUrl)
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Icon(
                                imageVector = Icons.Default.Public,
                                contentDescription = null,
                                tint = BioDexColors.Primary,
                                modifier = Modifier.size(16.dp),
                            )
                            Spacer(modifier = Modifier.width(6.dp))
                            Text("Wikipedia", color = BioDexColors.TextPrimary)
                        }
                    }

                    OutlinedButton(
                        onClick = {
                            BioDexFormatters.copyUrlToClipboard(
                                context,
                                item.commonName ?: item.scientificName,
                                naturalistaUrl,
                            )
                        },
                        modifier = Modifier.weight(1f),
                    ) {
                        Icon(
                            imageVector = Icons.Default.ContentCopy,
                            contentDescription = null,
                            tint = BioDexColors.Primary,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.width(6.dp))
                        Text("Copiar URL", color = BioDexColors.TextPrimary)
                    }
                }

                Text(
                    text = "URL: $naturalistaUrl · Taxon ${item.taxonId.source}:${item.taxonId.id}",
                    style = MaterialTheme.typography.labelSmall,
                    color = BioDexColors.TextSecondary,
                )
            }
        }
    }
}

@Composable
private fun SpeciesThumbnail(
    imageUrl: String?,
    contentDescription: String,
    unlocked: Boolean,
    sizeDp: Int,
) {
    val grayscaleFilter = remember(unlocked) {
        if (unlocked) {
            null
        } else {
            ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0.1f) })
        }
    }

    Box(
        modifier = Modifier
            .size(sizeDp.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(BioDexColors.SurfaceElevated),
        contentAlignment = Alignment.Center,
    ) {
        if (!imageUrl.isNullOrBlank()) {
            SubcomposeAsyncImage(
                model = imageUrl,
                contentDescription = contentDescription,
                contentScale = ContentScale.Crop,
                colorFilter = grayscaleFilter,
                alpha = if (unlocked) 1f else 0.45f,
                modifier = Modifier.fillMaxSize(),
                loading = {
                    CircularProgressIndicator(
                        color = BioDexColors.Primary,
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                    )
                },
                error = {
                    Icon(
                        imageVector = Icons.Default.Pets,
                        contentDescription = null,
                        tint = BioDexColors.TextSecondary,
                    )
                },
            )
        } else {
            Icon(
                imageVector = Icons.Default.Pets,
                contentDescription = null,
                tint = BioDexColors.TextSecondary,
            )
        }

        if (!unlocked) {
            Box(
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(4.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(Color(0xBB0B1410))
                    .padding(3.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Lock,
                    contentDescription = "Por descubrir",
                    tint = BioDexColors.TextSecondary,
                    modifier = Modifier.size(12.dp),
                )
            }
        }
    }
}

@Composable
private fun SpeciesHeroImage(
    imageUrl: String?,
    contentDescription: String,
    unlocked: Boolean,
    attribution: String?,
) {
    val grayscaleFilter = remember(unlocked) {
        if (unlocked) {
            null
        } else {
            ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0.15f) })
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(210.dp)
                .clip(RoundedCornerShape(14.dp))
                .background(BioDexColors.SurfaceElevated),
            contentAlignment = Alignment.Center,
        ) {
            if (!imageUrl.isNullOrBlank()) {
                SubcomposeAsyncImage(
                    model = imageUrl,
                    contentDescription = contentDescription,
                    contentScale = ContentScale.Crop,
                    colorFilter = grayscaleFilter,
                    alpha = if (unlocked) 1f else 0.55f,
                    modifier = Modifier.fillMaxSize(),
                    loading = {
                        CircularProgressIndicator(color = BioDexColors.Primary)
                    },
                    error = {
                        Icon(
                            imageVector = Icons.Default.Pets,
                            contentDescription = null,
                            tint = BioDexColors.TextSecondary,
                            modifier = Modifier.size(48.dp),
                        )
                    },
                )
            } else {
                Icon(
                    imageVector = Icons.Default.Pets,
                    contentDescription = null,
                    tint = BioDexColors.TextSecondary,
                    modifier = Modifier.size(48.dp),
                )
            }
        }
        if (!attribution.isNullOrBlank()) {
            Text(
                text = "Foto NaturaLista: $attribution",
                style = MaterialTheme.typography.labelSmall,
                color = BioDexColors.TextSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun DiscoveryBadge(
    discovered: Boolean,
    bestVariant: String?,
    compact: Boolean = false,
) {
    val color = if (discovered) BioDexColors.Primary else BioDexColors.TextSecondary
    val label = if (discovered) {
        when (bestVariant) {
            "wild" -> if (compact) "SALVAJE" else "CAPTURADO · SALVAJE"
            "sanctuary" -> if (compact) "SANTUARIO" else "CAPTURADO · SANTUARIO"
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
                maxLines = 1,
                softWrap = false,
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
            maxLines = 1,
            softWrap = false,
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
private fun CenteredLoader() {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(220.dp),
        contentAlignment = Alignment.Center,
    ) {
        CircularProgressIndicator(color = BioDexColors.Primary)
    }
}

@Composable
private fun ErrorBanner(message: String) {
    Surface(
        color = Color(0xCC7F1D1D),
        shape = RoundedCornerShape(14.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = message,
            color = Color.White,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.padding(14.dp),
        )
    }
}

@Composable
private fun CenteredEmptyText(text: String) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(200.dp)
            .padding(16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = text,
            color = BioDexColors.TextSecondary,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

private fun iucnAccentColor(status: String): Color = when (status.uppercase()) {
    "CR" -> Color(0xFFFF6B6B)
    "EN" -> Color(0xFFF59E0B)
    "VU" -> Color(0xFF60A5FA)
    "NT" -> Color(0xFF34D399)
    else -> BioDexColors.TextSecondary
}
