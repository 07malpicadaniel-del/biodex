package org.biodex.app

import org.biodex.app.data.BioDexApiClient
import org.junit.Assert.assertEquals
import org.junit.Test

class BioDexApiClientTest {
    @Test
    fun normalizesAndDeduplicatesCandidateEndpoints() {
        val candidates = BioDexApiClient.buildCandidateUrls(
            customUrl = "192.168.100.46:3000/",
            primaryUrl = "http://127.0.0.1:3000",
            fallbackUrl = "http://192.168.100.46:3000/",
            preferredActive = "http://127.0.0.1:3000/",
        )
        assertEquals(
            listOf("http://192.168.100.46:3000", "http://127.0.0.1:3000"),
            candidates,
        )
    }
}
