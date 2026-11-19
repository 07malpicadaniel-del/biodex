package org.biodex.app.integrity

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import androidx.exifinterface.media.ExifInterface
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream

data class PreparedCaptureImage(
    val uploadJpegBytes: ByteArray,
    val moireScore: Float,
)

object CaptureImageProcessor {
    private const val MAX_EDGE_PX = 1280
    private const val JPEG_QUALITY = 88

    fun prepareLiveCapture(rawJpegBytes: ByteArray): PreparedCaptureImage {
        val moireScore = MoireAnalyzer.estimateFromJpeg(rawJpegBytes)
        val normalized = normalizeOrientationAndSize(rawJpegBytes) ?: rawJpegBytes
        return PreparedCaptureImage(
            uploadJpegBytes = normalized,
            moireScore = moireScore,
        )
    }

    private fun normalizeOrientationAndSize(rawJpegBytes: ByteArray): ByteArray? {
        val rotationDegrees = runCatching {
            val exif = ExifInterface(ByteArrayInputStream(rawJpegBytes))
            when (exif.getAttributeInt(
                ExifInterface.TAG_ORIENTATION,
                ExifInterface.ORIENTATION_NORMAL,
            )) {
                ExifInterface.ORIENTATION_ROTATE_90 -> 90f
                ExifInterface.ORIENTATION_ROTATE_180 -> 180f
                ExifInterface.ORIENTATION_ROTATE_270 -> 270f
                else -> 0f
            }
        }.getOrDefault(0f)

        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(rawJpegBytes, 0, rawJpegBytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null

        val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
        if (maxDim <= MAX_EDGE_PX && rotationDegrees == 0f) {
            return rawJpegBytes
        }

        var sampleSize = 1
        while (maxDim / sampleSize > MAX_EDGE_PX * 2) {
            sampleSize *= 2
        }

        val decodeOpts = BitmapFactory.Options().apply { inSampleSize = sampleSize }
        val decoded = BitmapFactory.decodeByteArray(rawJpegBytes, 0, rawJpegBytes.size, decodeOpts)
            ?: return null

        val target = computeScaledDimensions(decoded.width, decoded.height, MAX_EDGE_PX)
        val matrix = Matrix().apply {
            if (target.first != decoded.width || target.second != decoded.height) {
                postScale(
                    target.first.toFloat() / decoded.width.toFloat(),
                    target.second.toFloat() / decoded.height.toFloat(),
                )
            }
            if (rotationDegrees != 0f) {
                postRotate(rotationDegrees)
            }
        }

        val transformed = Bitmap.createBitmap(
            decoded,
            0,
            0,
            decoded.width,
            decoded.height,
            matrix,
            true,
        )
        if (transformed !== decoded) {
            decoded.recycle()
        }

        return try {
            val out = ByteArrayOutputStream()
            transformed.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, out)
            out.toByteArray()
        } finally {
            transformed.recycle()
        }
    }

    fun computeScaledDimensions(width: Int, height: Int, maxEdge: Int = MAX_EDGE_PX): Pair<Int, Int> {
        if (width <= 0 || height <= 0) return 1 to 1
        val currentMax = maxOf(width, height)
        if (currentMax <= maxEdge) return width to height
        val scale = maxEdge.toFloat() / currentMax.toFloat()
        val newW = (width * scale).toInt().coerceAtLeast(1)
        val newH = (height * scale).toInt().coerceAtLeast(1)
        return newW to newH
    }
}
