package org.biodex.app

import kotlinx.serialization.json.Json
import org.biodex.app.data.AlmanacListResponse
import org.biodex.app.data.BioDexApiClient
import org.biodex.app.data.NaturaListaSyncResponse
import org.junit.Assert.assertEquals
import org.junit.Test

class BioDexApiClientTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun normalizesAndDeduplicatesCandidateEndpoints() {
        val candidates = BioDexApiClient.buildCandidateUrls(
            customUrl = "192.168.100.46:3030/",
            primaryUrl = "http://127.0.0.1:3030",
            fallbackUrl = "http://192.168.100.46:3030/",
            preferredActive = "http://127.0.0.1:3030/",
        )
        assertEquals(
            listOf("http://192.168.100.46:3030", "http://127.0.0.1:3030"),
            candidates,
        )
    }

    @Test
    fun decodesPaginatedAlmanacAndNaturaListaSyncPayloads() {
        val listPayload = """
            {
              "total_species": 920,
              "discovered_count": 920,
              "matched_count": 48,
              "category_counts": { "canids": 36, "felines": 38, "sharks": 48 },
              "entries": []
            }
        """.trimIndent()
        val parsedList = json.decodeFromString<AlmanacListResponse>(listPayload)
        assertEquals(920, parsedList.totalSpecies)
        assertEquals(48, parsedList.matchedCount)
        assertEquals(36, parsedList.categoryCounts["canids"])
        assertEquals(48, parsedList.categoryCounts["sharks"])

        val syncPayload = """
            {
              "page": 2,
              "fetched_candidates": 400,
              "filtered_non_species": 27,
              "upserted_species": 373,
              "total_catalog_species": 1293
            }
        """.trimIndent()
        val parsedSync = json.decodeFromString<NaturaListaSyncResponse>(syncPayload)
        assertEquals(2, parsedSync.page)
        assertEquals(27, parsedSync.filteredNonSpecies)
        assertEquals(1293L, parsedSync.totalCatalogSpecies)
    }
}
