package org.biodex.app

import org.biodex.app.integrity.MoireAnalyzer
import org.junit.Assert.assertTrue
import org.junit.Test

class MoireAnalyzerTest {
    @Test
    fun smoothGradientProducesLowMoireScore() {
        val width = 32
        val height = 32
        val grid = IntArray(width * height) { idx ->
            val x = idx % width
            val y = idx / width
            ((x * 4 + y * 4) % 255)
        }
        val score = MoireAnalyzer.estimateFromLumaGrid(grid, width, height)
        assertTrue("Expected low Moiré score for smooth gradient, got $score", score < 0.25f)
    }

    @Test
    fun alternatingCheckerboardProducesHighMoireScore() {
        val width = 32
        val height = 32
        val grid = IntArray(width * height) { idx ->
            val x = idx % width
            val y = idx / width
            if ((x + y) % 2 == 0) 220 else 20
        }
        val score = MoireAnalyzer.estimateFromLumaGrid(grid, width, height)
        assertTrue("Expected high Moiré score for pixel grid, got $score", score > 0.60f)
    }
}
