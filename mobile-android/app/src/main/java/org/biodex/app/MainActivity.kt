package org.biodex.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import org.biodex.app.ui.AlmanacDetailScreen
import org.biodex.app.ui.AlmanacScreen
import org.biodex.app.ui.BioDexColors
import org.biodex.app.ui.BioDexTab
import org.biodex.app.ui.BioDexTheme
import org.biodex.app.ui.BioDexViewModel
import org.biodex.app.ui.CameraScannerScreen
import org.biodex.app.ui.CardResultScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            BioDexTheme {
                val vm: BioDexViewModel = viewModel()
                val uiState by vm.uiState.collectAsState()
                val captureResponse = uiState.lastCaptureResponse
                val detailEntry = uiState.selectedAlmanacEntry

                when {
                    captureResponse != null -> {
                        CardResultScreen(
                            response = captureResponse,
                            capturedJpegBytes = uiState.capturedJpegBytes,
                            onScanAnother = vm::clearResult,
                        )
                    }

                    detailEntry != null -> {
                        AlmanacDetailScreen(
                            item = detailEntry,
                            onBack = vm::closeAlmanacDetail,
                        )
                    }

                    else -> {
                        Scaffold(
                            containerColor = BioDexColors.Background,
                            bottomBar = {
                                NavigationBar(containerColor = BioDexColors.Surface) {
                                    NavigationBarItem(
                                        selected = uiState.activeTab == BioDexTab.Scanner,
                                        onClick = { vm.selectTab(BioDexTab.Scanner) },
                                        icon = {
                                            Icon(
                                                imageVector = Icons.Default.CameraAlt,
                                                contentDescription = "Escáner",
                                            )
                                        },
                                        label = { Text("Escáner") },
                                        colors = NavigationBarItemDefaults.colors(
                                            selectedIconColor = BioDexColors.OnPrimary,
                                            selectedTextColor = BioDexColors.Primary,
                                            indicatorColor = BioDexColors.Primary,
                                            unselectedIconColor = BioDexColors.TextSecondary,
                                            unselectedTextColor = BioDexColors.TextSecondary,
                                        ),
                                    )
                                    NavigationBarItem(
                                        selected = uiState.activeTab == BioDexTab.Almanac,
                                        onClick = { vm.selectTab(BioDexTab.Almanac) },
                                        icon = {
                                            Icon(
                                                imageVector = Icons.AutoMirrored.Filled.MenuBook,
                                                contentDescription = "Almanaque",
                                            )
                                        },
                                        label = { Text("Almanaque") },
                                        colors = NavigationBarItemDefaults.colors(
                                            selectedIconColor = BioDexColors.OnPrimary,
                                            selectedTextColor = BioDexColors.Primary,
                                            indicatorColor = BioDexColors.Primary,
                                            unselectedIconColor = BioDexColors.TextSecondary,
                                            unselectedTextColor = BioDexColors.TextSecondary,
                                        ),
                                    )
                                }
                            },
                        ) { innerPadding ->
                            when (uiState.activeTab) {
                                BioDexTab.Scanner -> CameraScannerScreen(
                                    uiState = uiState,
                                    onLocationPermissionGranted = vm::onLocationPermissionGranted,
                                    onRefreshHealth = vm::refreshBackendHealth,
                                    onCaptureJpeg = vm::submitCameraCapture,
                                    onDismissError = vm::clearError,
                                    modifier = Modifier.padding(bottom = innerPadding.calculateBottomPadding()),
                                )

                                BioDexTab.Almanac -> AlmanacScreen(
                                    uiState = uiState,
                                    onSearchChange = vm::updateAlmanacSearch,
                                    onSelectCategory = vm::selectAlmanacCategory,
                                    onRefresh = vm::refreshAlmanac,
                                    onOpenDetail = vm::openAlmanacDetail,
                                    modifier = Modifier.padding(bottom = innerPadding.calculateBottomPadding()),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
