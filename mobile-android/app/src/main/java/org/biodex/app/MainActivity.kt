package org.biodex.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import org.biodex.app.ui.AlmanacDetailScreen
import org.biodex.app.ui.AlmanacScreen
import org.biodex.app.ui.BattleScreen
import org.biodex.app.ui.BioDexColors
import org.biodex.app.ui.BioDexTab
import org.biodex.app.ui.BioDexTheme
import org.biodex.app.ui.BioDexViewModel
import org.biodex.app.ui.CameraScannerScreen
import org.biodex.app.ui.CardResultScreen
import org.biodex.app.ui.FriendsScreen
import org.biodex.app.ui.LoginScreen
import org.biodex.app.ui.ProfileScreen

private data class NavTabItem(
    val tab: BioDexTab,
    val label: String,
    val icon: ImageVector,
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        val navItems = listOf(
            NavTabItem(BioDexTab.Scanner, "Escáner", Icons.Default.CameraAlt),
            NavTabItem(BioDexTab.Collection, "Colección", Icons.AutoMirrored.Filled.MenuBook),
            NavTabItem(BioDexTab.Battle, "Combate", Icons.Default.Bolt),
            NavTabItem(BioDexTab.Friends, "Amigos", Icons.Default.Group),
            NavTabItem(BioDexTab.Profile, "Perfil", Icons.Default.Person),
        )

        setContent {
            BioDexTheme {
                val vm: BioDexViewModel = viewModel()
                val uiState by vm.uiState.collectAsState()
                val captureResponse = uiState.lastCaptureResponse
                val detailEntry = uiState.selectedAlmanacEntry

                when {
                    !uiState.isLoggedIn -> {
                        LoginScreen(
                            uiState = uiState,
                            onExplorerNameChange = vm::updateExplorerNameInput,
                            onToggleDevMode = vm::setLoginDevModeSelected,
                            onCustomUrlChange = vm::updateCustomServerUrlInput,
                            onApplyCustomUrl = vm::applyCustomServerUrl,
                            onRefreshHealth = vm::refreshConnectionHealthOnly,
                            onLogin = vm::login,
                        )
                    }

                    captureResponse != null -> {
                        CardResultScreen(
                            response = captureResponse,
                            capturedJpegBytes = uiState.capturedJpegBytes,
                            onScanAnother = vm::clearResult,
                            onOpenCollection = vm::goToCollectionFromResult,
                        )
                    }

                    detailEntry != null -> {
                        AlmanacDetailScreen(
                            item = detailEntry,
                            isDevMode = uiState.isDevMode,
                            onBack = vm::closeAlmanacDetail,
                        )
                    }

                    else -> {
                        Scaffold(
                            containerColor = BioDexColors.Background,
                            bottomBar = {
                                NavigationBar(containerColor = BioDexColors.Surface) {
                                    navItems.forEach { item ->
                                        NavigationBarItem(
                                            selected = uiState.activeTab == item.tab,
                                            onClick = { vm.selectTab(item.tab) },
                                            icon = {
                                                Icon(
                                                    imageVector = item.icon,
                                                    contentDescription = item.label,
                                                )
                                            },
                                            label = {
                                                Text(
                                                    text = item.label,
                                                    fontSize = 11.sp,
                                                    maxLines = 1,
                                                    softWrap = false,
                                                )
                                            },
                                            alwaysShowLabel = true,
                                            colors = NavigationBarItemDefaults.colors(
                                                selectedIconColor = BioDexColors.OnPrimary,
                                                selectedTextColor = BioDexColors.Primary,
                                                indicatorColor = BioDexColors.Primary,
                                                unselectedIconColor = BioDexColors.TextSecondary,
                                                unselectedTextColor = BioDexColors.TextSecondary,
                                            ),
                                        )
                                    }
                                }
                            },
                        ) { innerPadding ->
                            val contentModifier =
                                Modifier.padding(bottom = innerPadding.calculateBottomPadding())
                            when (uiState.activeTab) {
                                BioDexTab.Scanner -> CameraScannerScreen(
                                    uiState = uiState,
                                    onLocationPermissionGranted = vm::onLocationPermissionGranted,
                                    onRefreshHealth = vm::refreshBackendHealth,
                                    onCaptureJpeg = vm::submitCameraCapture,
                                    onCaptureError = vm::reportCaptureError,
                                    onDismissError = vm::clearError,
                                    modifier = contentModifier,
                                )

                                BioDexTab.Collection -> AlmanacScreen(
                                    uiState = uiState,
                                    onSelectSubTab = vm::selectCollectionSubTab,
                                    onSearchChange = vm::updateAlmanacSearch,
                                    onSelectCategory = vm::selectAlmanacCategory,
                                    onSelectCaptureFilter = vm::selectAlmanacCaptureFilter,
                                    onRefresh = vm::refreshAlmanac,
                                    onOpenDetail = vm::openAlmanacDetail,
                                    onNaturaListaQueryChange = vm::updateNaturaListaSearchQuery,
                                    onSubmitNaturaListaSearch = vm::submitNaturaListaSearch,
                                    onRefreshNearbyNaturaLista = vm::refreshNearbyNaturaLista,
                                    onInspectNaturaListaSpecies = vm::inspectNaturaListaSpecies,
                                    modifier = contentModifier,
                                )

                                BioDexTab.Battle -> BattleScreen(
                                    onGoToScanner = { vm.selectTab(BioDexTab.Scanner) },
                                    onGoToCollection = { vm.selectTab(BioDexTab.Collection) },
                                    modifier = contentModifier,
                                )

                                BioDexTab.Friends -> FriendsScreen(
                                    uiState = uiState,
                                    modifier = contentModifier,
                                )

                                BioDexTab.Profile -> ProfileScreen(
                                    uiState = uiState,
                                    onCustomUrlChange = vm::updateCustomServerUrlInput,
                                    onApplyCustomUrl = vm::applyCustomServerUrl,
                                    onResetDefaultUrl = { vm.updateCustomServerUrlInput("") },
                                    onRefreshConnection = vm::applyCustomServerUrl,
                                    onLogout = vm::logout,
                                    modifier = contentModifier,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
