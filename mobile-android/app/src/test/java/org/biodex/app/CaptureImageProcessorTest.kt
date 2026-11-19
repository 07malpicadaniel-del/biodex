package org.biodex.app

import org.biodex.app.integrity.CaptureImageProcessor
import org.junit.Assert.assertEquals
import org.junit.Test

class CaptureImageProcessorTest {
    @Test
    fun scalesOversizedImagesProportionallyToMaxEdge() {
        assertEquals(1280 to 960, CaptureImageProcessor.computeScaledDimensions(4000, 3000, 1280))
        assertEquals(720 to 1280, CaptureImageProcessor.computeScaledDimensions(2250, 4000, 1280))
        assertEquals(800 to 600, CaptureImageProcessor.computeScaledDimensions(800, 600, 1280))
    }
}
