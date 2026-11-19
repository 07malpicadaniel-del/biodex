package org.biodex.app.ui

import android.Manifest
import android.annotation.SuppressLint
import android.content.pm.PackageManager
import android.util.Size
import android.view.MotionEvent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.FocusMeteringAction
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.AspectRatioStrategy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.FlashOff
import androidx.compose.material.icons.filled.FlashOn
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.ZoomIn
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import kotlin.math.roundToInt

@SuppressLint("ClickableViewAccessibility")
@Composable
fun CameraScannerScreen(
    uiState: BioDexUiState,
    onLocationPermissionGranted: () -> Unit,
    onRefreshHealth: () -> Unit,
    onCaptureJpeg: (ByteArray) -> Unit,
    onCaptureError: (String) -> Unit,
    onDismissError: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED,
        )
    }
    var hasLocationPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_FINE_LOCATION) ==
                PackageManager.PERMISSION_GRANTED,
        )
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        hasCameraPermission = grants[Manifest.permission.CAMERA] == true
        val locGranted = grants[Manifest.permission.ACCESS_FINE_LOCATION] == true ||
            grants[Manifest.permission.ACCESS_COARSE_LOCATION] == true
        hasLocationPermission = locGranted
        if (locGranted) {
            onLocationPermissionGranted()
        }
    }

    LaunchedEffect(Unit) {
        if (hasLocationPermission) {
            onLocationPermissionGranted()
        }
        if (!hasCameraPermission || !hasLocationPermission) {
            permissionLauncher.launch(
                arrayOf(
                    Manifest.permission.CAMERA,
                    Manifest.permission.ACCESS_FINE_LOCATION,
                    Manifest.permission.ACCESS_COARSE_LOCATION,
                ),
            )
        }
    }

    val captureExecutor = remember { Executors.newSingleThreadExecutor() }
    DisposableEffect(Unit) {
        onDispose { captureExecutor.shutdown() }
    }

    var imageCapture by remember { mutableStateOf<ImageCapture?>(null) }
    var boundCamera by remember { mutableStateOf<Camera?>(null) }
    var torchEnabled by remember { mutableStateOf(false) }
    var zoom2xEnabled by remember { mutableStateOf(false) }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(BioDexColors.Background),
    ) {
        if (hasCameraPermission) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { ctx ->
                    val previewView = PreviewView(ctx).apply {
                        scaleType = PreviewView.ScaleType.FILL_CENTER
                    }
                    val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)
                    cameraProviderFuture.addListener(
                        {
                            val cameraProvider = cameraProviderFuture.get()
                            val preview = Preview.Builder().build().also {
                                it.setSurfaceProvider(previewView.surfaceProvider)
                            }
                            val resolutionSelector = ResolutionSelector.Builder()
                                .setAspectRatioStrategy(AspectRatioStrategy.RATIO_4_3_FALLBACK_AUTO_STRATEGY)
                                .setResolutionStrategy(
                                    ResolutionStrategy(
                                        Size(1280, 960),
                                        ResolutionStrategy.FALLBACK_RULE_CLOSEST_LOWER_THEN_HIGHER,
                                    ),
                                )
                                .build()
                            val capture = ImageCapture.Builder()
                                .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
                                .setJpegQuality(90)
                                .setResolutionSelector(resolutionSelector)
                                .build()

                            cameraProvider.unbindAll()
                            val cam = cameraProvider.bindToLifecycle(
                                lifecycleOwner,
                                CameraSelector.DEFAULT_BACK_CAMERA,
                                preview,
                                capture,
                            )
                            boundCamera = cam
                            imageCapture = capture

                            previewView.setOnTouchListener { view, event ->
                                if (event.action == MotionEvent.ACTION_UP) {
                                    val point = previewView.meteringPointFactory.createPoint(event.x, event.y)
                                    val action = FocusMeteringAction.Builder(point, FocusMeteringAction.FLAG_AF)
                                        .setAutoCancelDuration(3, TimeUnit.SECONDS)
                                        .build()
                                    cam.cameraControl.startFocusAndMetering(action)
                                    view.performClick()
                                }
                                true
                            }
                        },
                        ContextCompat.getMainExecutor(ctx),
                    )
                    previewView
                },
            )

            // Viewfinder framing reticle
            ScannerReticleOverlay(
                modifier = Modifier
                    .align(Alignment.Center)
                    .size(236.dp),
                accent = if (uiState.isSubmitting) BioDexColors.Warning else BioDexColors.Primary,
            )
        } else {
            Column(
                modifier = Modifier
                    .align(Alignment.Center)
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "Se requiere cámara en vivo",
                    style = MaterialTheme.typography.titleLarge,
                    color = BioDexColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = "BioDex captura únicamente en vivo con CameraX para garantizar autenticidad.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = BioDexColors.TextSecondary,
                    textAlign = TextAlign.Center,
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = {
                        permissionLauncher.launch(
                            arrayOf(
                                Manifest.permission.CAMERA,
                                Manifest.permission.ACCESS_FINE_LOCATION,
                            ),
                        )
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = BioDexColors.Primary),
                ) {
                    Text("Conceder Permisos", color = BioDexColors.OnPrimary)
                }
            }
        }

        // Top Telemetry HUD
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .statusBarsPadding()
                .padding(16.dp)
                .align(Alignment.TopCenter),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Surface(
                color = Color(0xCC0A1510),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Text(
                                text = "BioDex Live Scanner",
                                style = MaterialTheme.typography.titleMedium,
                                color = BioDexColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                            )
                            if (uiState.isDevMode) {
                                Surface(
                                    color = BioDexColors.Warning.copy(alpha = 0.22f),
                                    shape = RoundedCornerShape(6.dp),
                                ) {
                                    Text(
                                        text = "DEV",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = BioDexColors.Warning,
                                        fontWeight = FontWeight.ExtraBold,
                                        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
                                    )
                                }
                            }
                        }
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            val dotColor = when (uiState.backendOnline) {
                                true -> BioDexColors.Primary
                                false -> BioDexColors.Danger
                                null -> BioDexColors.Warning
                            }
                            Box(
                                modifier = Modifier
                                    .size(8.dp)
                                    .clip(CircleShape)
                                    .background(dotColor),
                            )
                            Text(
                                text = uiState.backendStatusText,
                                style = MaterialTheme.typography.labelMedium,
                                color = BioDexColors.TextSecondary,
                            )
                        }
                    }

                    IconButton(onClick = onRefreshHealth) {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Reconectar backend",
                            tint = BioDexColors.Primary,
                        )
                    }
                }
            }

            // Hardware integrity badges (ECDSA P-256 + IMU + Real GPS)
            Surface(
                color = Color(0xBB0A1510),
                shape = RoundedCornerShape(12.dp),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            imageVector = Icons.Default.Lock,
                            contentDescription = null,
                            tint = BioDexColors.Primary,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            text = "P-256 · IMU ${if (uiState.telemetry.hasImuFix) "OK" else "..."}",
                            style = MaterialTheme.typography.labelSmall,
                            color = BioDexColors.TextPrimary,
                        )
                    }

                    val activeLat = uiState.telemetry.latitude
                    val activeLon = uiState.telemetry.longitude
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            imageVector = Icons.Default.LocationOn,
                            contentDescription = null,
                            tint = if (activeLat != null) BioDexColors.Primary else BioDexColors.Warning,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            text = if (activeLat != null && activeLon != null) {
                                val acc = uiState.telemetry.accuracyMeters.roundToInt()
                                "%.4f, %.4f (±%dm)".format(activeLat, activeLon, acc)
                            } else {
                                "Buscando GPS..."
                            },
                            style = MaterialTheme.typography.labelSmall,
                            color = BioDexColors.TextPrimary,
                        )
                    }
                }
            }
        }

        // Camera quick controls (Torch + Zoom) on right edge
        if (hasCameraPermission) {
            Column(
                modifier = Modifier
                    .align(Alignment.CenterEnd)
                    .padding(end = 14.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                if (boundCamera?.cameraInfo?.hasFlashUnit() == true) {
                    Surface(
                        color = Color(0xBB0A1510),
                        shape = CircleShape,
                        modifier = Modifier
                            .size(44.dp)
                            .border(
                                1.dp,
                                if (torchEnabled) BioDexColors.Warning else BioDexColors.SurfaceElevated,
                                CircleShape,
                            )
                            .clickable {
                                val next = !torchEnabled
                                torchEnabled = next
                                boundCamera?.cameraControl?.enableTorch(next)
                            },
                    ) {
                        Box(contentAlignment = Alignment.Center) {
                            Icon(
                                imageVector = if (torchEnabled) Icons.Default.FlashOn else Icons.Default.FlashOff,
                                contentDescription = "Linterna",
                                tint = if (torchEnabled) BioDexColors.Warning else BioDexColors.TextPrimary,
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }

                Surface(
                    color = Color(0xBB0A1510),
                    shape = CircleShape,
                    modifier = Modifier
                        .size(44.dp)
                        .border(
                            1.dp,
                            if (zoom2xEnabled) BioDexColors.Primary else BioDexColors.SurfaceElevated,
                            CircleShape,
                        )
                        .clickable {
                            val nextZoom = !zoom2xEnabled
                            zoom2xEnabled = nextZoom
                            val zoomState = boundCamera?.cameraInfo?.zoomState?.value
                            val targetRatio = if (nextZoom && zoomState != null) {
                                2.0f.coerceIn(zoomState.minZoomRatio, zoomState.maxZoomRatio)
                            } else {
                                zoomState?.minZoomRatio ?: 1.0f
                            }
                            boundCamera?.cameraControl?.setZoomRatio(targetRatio)
                        },
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        if (zoom2xEnabled) {
                            Text(
                                text = "2x",
                                style = MaterialTheme.typography.labelMedium,
                                color = BioDexColors.Primary,
                                fontWeight = FontWeight.ExtraBold,
                            )
                        } else {
                            Icon(
                                imageVector = Icons.Default.ZoomIn,
                                contentDescription = "Zoom 2x",
                                tint = BioDexColors.TextPrimary,
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }
            }
        }

        // Bottom Live Shutter Controls
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .align(Alignment.BottomCenter)
                .padding(horizontal = 16.dp, vertical = 20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (uiState.isSubmitting) {
                Surface(
                    color = Color(0xE60A1510),
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        CircularProgressIndicator(
                            color = BioDexColors.Primary,
                            modifier = Modifier.size(20.dp),
                            strokeWidth = 2.5.dp,
                        )
                        Text(
                            text = "Identificando especie con NaturaLista CV y validando rango PostGIS...",
                            style = MaterialTheme.typography.bodySmall,
                            color = BioDexColors.TextPrimary,
                        )
                    }
                }
            }

            uiState.errorMessage?.let { msg ->
                Surface(
                    color = Color(0xE67F1D1D),
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onDismissError() },
                ) {
                    Text(
                        text = msg,
                        color = Color.White,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(12.dp),
                    )
                }
            }

            Box(
                modifier = Modifier
                    .size(78.dp)
                    .clip(CircleShape)
                    .border(3.dp, BioDexColors.Primary, CircleShape)
                    .background(Color(0xCC13241C))
                    .clickable(enabled = hasCameraPermission && !uiState.isSubmitting && imageCapture != null) {
                        val captureUseCase = imageCapture ?: return@clickable
                        captureUseCase.takePicture(
                            captureExecutor,
                            object : ImageCapture.OnImageCapturedCallback() {
                                override fun onCaptureSuccess(image: ImageProxy) {
                                    val bytes = try {
                                        val buffer = image.planes[0].buffer
                                        ByteArray(buffer.remaining()).also { buffer.get(it) }
                                    } finally {
                                        image.close()
                                    }
                                    onCaptureJpeg(bytes)
                                }

                                override fun onError(exception: ImageCaptureException) {
                                    onCaptureError(
                                        "No se pudo capturar la imagen (${exception.message ?: "error de cámara"})",
                                    )
                                }
                            },
                        )
                    },
                contentAlignment = Alignment.Center,
            ) {
                if (uiState.isSubmitting) {
                    CircularProgressIndicator(
                        color = BioDexColors.Primary,
                        modifier = Modifier.size(34.dp),
                        strokeWidth = 3.dp,
                    )
                } else {
                    Icon(
                        imageVector = Icons.Default.CameraAlt,
                        contentDescription = "Capturar especie",
                        tint = BioDexColors.Primary,
                        modifier = Modifier.size(34.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun ScannerReticleOverlay(
    accent: Color,
    modifier: Modifier = Modifier,
) {
    Canvas(modifier = modifier) {
        val cornerLen = size.minDimension * 0.16f
        val stroke = 3.dp.toPx()

        // Top-Left
        drawLine(accent, Offset(0f, 0f), Offset(cornerLen, 0f), stroke, StrokeCap.Round)
        drawLine(accent, Offset(0f, 0f), Offset(0f, cornerLen), stroke, StrokeCap.Round)
        // Top-Right
        drawLine(accent, Offset(size.width, 0f), Offset(size.width - cornerLen, 0f), stroke, StrokeCap.Round)
        drawLine(accent, Offset(size.width, 0f), Offset(size.width, cornerLen), stroke, StrokeCap.Round)
        // Bottom-Left
        drawLine(accent, Offset(0f, size.height), Offset(cornerLen, size.height), stroke, StrokeCap.Round)
        drawLine(accent, Offset(0f, size.height), Offset(0f, size.height - cornerLen), stroke, StrokeCap.Round)
        // Bottom-Right
        drawLine(accent, Offset(size.width, size.height), Offset(size.width - cornerLen, size.height), stroke, StrokeCap.Round)
        drawLine(accent, Offset(size.width, size.height), Offset(size.width, size.height - cornerLen), stroke, StrokeCap.Round)
    }
}
