package org.biodex.app.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import org.biodex.app.data.TaxonIdDto
import java.text.NumberFormat
import java.util.Locale

internal object BioDexFormatters {
    fun formatMass(grams: Long): String = when {
        grams >= 1_000_000L -> "%.1f t".format(grams / 1_000_000.0)
        grams >= 1_000L -> "%.1f kg".format(grams / 1_000.0)
        else -> "$grams g"
    }

    fun formatObservations(count: Long): String =
        NumberFormat.getIntegerInstance(Locale("es", "MX")).format(count)

    fun iucnLabelEs(code: String): String = when (code.uppercase()) {
        "LC" -> "Preocupación Menor (LC)"
        "NT" -> "Casi Amenazada (NT)"
        "VU" -> "Vulnerable (VU)"
        "EN" -> "En Peligro (EN)"
        "CR" -> "En Peligro Crítico (CR)"
        else -> "Sin evaluar"
    }

    fun iucnShortCode(code: String): String = when (code.uppercase()) {
        "LC", "NT", "VU", "EN", "CR" -> "UICN ${code.uppercase()}"
        else -> "UICN N/D"
    }

    fun habitatLabelEs(habitat: String): String = when (habitat.lowercase()) {
        "aerial" -> "Aéreo"
        "terrestrial" -> "Terrestre"
        "freshwater" -> "Dulceacuícola"
        "marine" -> "Marino"
        "abyssal" -> "Abisal"
        else -> habitat
    }

    fun dietLabelEs(diet: String): String = when (diet.lowercase()) {
        "carnivore" -> "Carnívoro"
        "herbivore" -> "Herbívoro"
        "omnivore" -> "Omnívoro"
        "detritivore" -> "Detritívoro"
        else -> diet
    }

    fun defenseLabelEs(defense: String): String = when (defense.lowercase()) {
        "venom" -> "Veneno"
        "camouflage" -> "Camuflaje"
        "shell_armor" -> "Caparazón"
        "spines" -> "Espinas"
        "ink" -> "Tinta"
        "speed" -> "Velocidad"
        else -> defense
    }

    fun naturalistaUrl(taxonId: TaxonIdDto, rawUrl: String? = null): String {
        if (!rawUrl.isNullOrBlank()) return rawUrl
        return when (taxonId.source) {
            "gbif" -> "https://www.gbif.org/species/${taxonId.id}"
            else -> "https://www.naturalista.mx/taxa/${taxonId.id}"
        }
    }

    fun openExternalUrl(context: Context, url: String) {
        val result = runCatching {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
        }
        if (result.isFailure) {
            Toast.makeText(context, "No se encontró navegador para abrir el enlace", Toast.LENGTH_SHORT).show()
        }
    }

    fun copyUrlToClipboard(context: Context, label: String, url: String) {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (clipboard != null) {
            clipboard.setPrimaryClip(ClipData.newPlainText(label, url))
            Toast.makeText(context, "Enlace copiado: $url", Toast.LENGTH_SHORT).show()
        }
    }
}
