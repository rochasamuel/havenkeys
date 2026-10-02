package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.content.pm.PackageManager
import uniffi.havenkeys_mobile.TargetFacts

/** Reads a confirmed save: per fill context, the form and only its fields' values. */
class SaveRequestReader(private val pm: PackageManager) {
    fun read(structures: List<AssistStructure>): List<SubmittedForm> = structures.mapNotNull { structure ->
        val parser = StructureParser()
        val screen = parser.parse(structure)
        if (screen.packageName.isEmpty()) return@mapNotNull null
        val save = SaveFormFinder.find(screen.fields, LoginFormFinder.find(screen.fields)) ?: return@mapNotNull null
        fun idOf(index: Int?) = index?.let(screen.ids::getOrNull)
        val ids = listOfNotNull(idOf(save.username), idOf(save.password), idOf(save.current)).toSet()
        val text = parser.textOf(structure, ids)
        SubmittedForm(
            TargetFacts(
                screen.packageName,
                CallerIdentity(pm).certDigests(screen.packageName),
                save.webDomain,
                save.webScheme,
            ),
            idOf(save.username)?.let(text::get),
            idOf(save.password)?.let(text::get),
            idOf(save.current)?.let(text::get),
        )
    }

    /** The app's name for a new app login; Rust falls back to the package. */
    @Suppress("SwallowedException")
    fun appTitle(packageName: String): String? = try {
        pm.getApplicationLabel(pm.getApplicationInfo(packageName, 0)).toString().take(MAX_TITLE)
    } catch (e: PackageManager.NameNotFoundException) {
        null
    }

    private companion object {
        const val MAX_TITLE = 100
    }
}
