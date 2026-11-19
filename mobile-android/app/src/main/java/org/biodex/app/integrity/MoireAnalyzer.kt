package org.biodex.app.integrity

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import kotlin.math.abs

/**
 * Lightweight anti-Moiré heuristic estimating periodic screen-grid artifacts in `0.0f..1.0f`.
 * Natural photographs exhibit low periodic pixel-to-pixel sign flips compared to photographed LCD/OLED grids.
 */
object MoireAnalyzer {
    fun estimateFromJpeg(jpegBytes: ByteArray): Float {
        val opts = BitmapFactory.Options().apply { inSampleSize = 4 }
        val bitmap = BitmapFactory.decodeByteArray(jpegBytes, 0, jpegBytes.size, opts)
            ?: return 0.10f

        return try {
            val cropSize = 96.coerceAtMost(minOf(bitmap.width, bitmap.height))
            if (cropSize < 8) return 0.05f
            val startX = (bitmap.width - cropSize) / 2
            val startY = (bitmap.height - cropSize) / 2
            val pixels = IntArray(cropSize * cropSize)
            bitmap.getPixels(pixels, 0, cropSize, startX, startY, cropSize, cropSize)

            val luma = IntArray(pixels.size) { idx ->
                val c = pixels[idx]
                val r = (c shr 16) and 0xFF
                val g = (c shr 8) and 0xFF
                val b = c and 0xFF
                (r * 299 + g * 587 + b * 114) / 1000
            }
            estimateFromLumaGrid(luma, cropSize, cropSize)
        } finally {
            bitmap.recycle()
        }
    }

    /**
     * Pure JVM grid analyzer measuring alternating sign changes in second-order luminance differences.
     */
    fun estimateFromLumaGrid(luma: IntArray, width: Int, height: Int): Float {
        if (width < 4 || height < 4 || luma.size < width * height) return 0.0f

        var highFreqOscillations = 0
        var samples = 0

        for (y in 1 until height - 1) {
            val rowOffset = y * width
            for (x in 1 until width - 1) {
                val center = luma[rowOffset + x]
                val left = luma[rowOffset + x - 1]
                val right = luma[rowOffset + x + 1]
                val up = luma[(y - 1) * width + x]
                val down = luma[(y + 1) * width + x]

                val horizOsc = (center - left) * (center - right) > 0 &&
                    abs(center - left) >= 14 &&
                    abs(center - right) >= 14
                val vertOsc = (center - up) * (center - down) > 0 &&
                    abs(center - up) >= 14 &&
                    abs(center - down) >= 14

                if (horizOsc || vertOsc) {
                    highFreqOscillations++
                }
                samples++
            }
        }

        if (samples == 0) return 0.0f
        val ratio = highFreqOscillations.toFloat() / samples.toFloat()
        return (ratio * 1.4f).coerceIn(0.0f, 1.0f)
    }
}
