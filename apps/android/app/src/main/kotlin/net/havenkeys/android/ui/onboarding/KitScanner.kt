package net.havenkeys.android.ui.onboarding

import android.Manifest
import android.content.pm.PackageManager
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.RequestPermission
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.LumaFrame

/**
 * The back camera, handing each frame's brightness plane to [onFrame]. The
 * camera permission is asked for here, when the user chose to scan, and
 * nowhere else. [cameraNeeded] and [cameraUnavailable] say what to do
 * instead of scanning; they default to the Recovery Sheet's words.
 */
@Composable
fun KitScanner(
    onFrame: (LumaFrame) -> Unit,
    modifier: Modifier = Modifier,
    cameraNeeded: String = stringResource(R.string.onboarding_camera_needed),
    cameraUnavailable: String = stringResource(R.string.onboarding_camera_unavailable),
) {
    val context = LocalContext.current
    var granted by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED,
        )
    }
    var asked by remember { mutableStateOf(false) }
    val request = rememberLauncherForActivityResult(RequestPermission()) {
        granted = it
        asked = true
    }
    LaunchedEffect(Unit) { if (!granted) request.launch(Manifest.permission.CAMERA) }

    when {
        granted -> CameraFrames(onFrame, cameraUnavailable, modifier)
        asked -> ScannerMessage(
            text = cameraNeeded,
            action = stringResource(R.string.onboarding_camera_allow),
            onAction = { request.launch(Manifest.permission.CAMERA) },
            modifier = modifier,
        )
        else -> Box(modifier)
    }
}

@Composable
private fun CameraFrames(onFrame: (LumaFrame) -> Unit, unavailable: String, modifier: Modifier) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val previewView = remember { PreviewView(context) }
    val currentOnFrame by rememberUpdatedState(onFrame)
    var failed by remember { mutableStateOf(false) }

    DisposableEffect(lifecycleOwner) {
        val analyzerThread = Executors.newSingleThreadExecutor()
        val preview = Preview.Builder().build().also { it.setSurfaceProvider(previewView.surfaceProvider) }
        val analysis = ImageAnalysis.Builder()
            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
            .setResolutionSelector(
                ResolutionSelector.Builder()
                    .setResolutionStrategy(
                        ResolutionStrategy(
                            Size(ANALYSIS_WIDTH, ANALYSIS_HEIGHT),
                            ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER,
                        ),
                    )
                    .build(),
            )
            .build()
        analysis.setAnalyzer(analyzerThread) { image -> image.use { currentOnFrame(it.toLumaFrame()) } }

        val future = ProcessCameraProvider.getInstance(context)
        var provider: ProcessCameraProvider? = null
        var disposed = false
        future.addListener(
            {
                if (disposed) return@addListener
                try {
                    provider = future.get().also {
                        it.bindToLifecycle(lifecycleOwner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                    }
                } catch (@Suppress("SwallowedException") e: ExecutionException) {
                    failed = true
                } catch (@Suppress("SwallowedException") e: IllegalArgumentException) {
                    // No back camera.
                    failed = true
                } catch (@Suppress("SwallowedException") e: IllegalStateException) {
                    failed = true
                }
            },
            ContextCompat.getMainExecutor(context),
        )
        onDispose {
            disposed = true
            provider?.unbind(preview, analysis)
            analysis.clearAnalyzer()
            analyzerThread.shutdown()
        }
    }

    if (failed) {
        ScannerMessage(text = unavailable, modifier = modifier)
    } else {
        AndroidView(factory = { previewView }, modifier = modifier)
    }
}

@Composable
private fun ScannerMessage(
    text: String,
    modifier: Modifier,
    action: String? = null,
    onAction: () -> Unit = {},
) {
    Box(modifier.padding(24.dp), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            HavenText(
                text,
                style = HavenTheme.type.body.copy(textAlign = TextAlign.Center),
                color = HavenTheme.colors.text,
            )
            if (action != null) HavenButton(action, onClick = onAction, style = ButtonStyle.Secondary)
        }
    }
}

/** Plane 0 of a YUV frame, copied row by row without the stride padding. */
private fun ImageProxy.toLumaFrame(): LumaFrame {
    val plane = planes[0]
    val buffer = plane.buffer
    val out = ByteArray(width * height)
    for (row in 0 until height) {
        buffer.position(row * plane.rowStride)
        buffer.get(out, row * width, width)
    }
    return LumaFrame(width.toUInt(), height.toUInt(), out)
}

// Enough pixels for the kit's dense QR code when held at arm's length.
private const val ANALYSIS_WIDTH = 1280
private const val ANALYSIS_HEIGHT = 720
