package app.keyflow.mobile.autofill

import android.app.assist.AssistStructure
import android.text.InputType
import android.view.View
import android.view.autofill.AutofillId

/** One detected fillable field and how confident/why it was identified as username or password. */
data class DetectedField(val autofillId: AutofillId, val kind: FieldKind)

enum class FieldKind { USERNAME, PASSWORD }

data class ParsedForm(val webDomain: String?, val packageName: String?, val fields: List<DetectedField>)

/**
 * Walks an [AssistStructure] looking for username/password fields, using
 * several independent signals and taking the first that matches —
 * mirroring the browser extension's own "multi-signal form detection"
 * philosophy (see `browser-extension/chrome/src/content.ts`) ported to
 * Android's very different, un-styleable view-tree format:
 *
 *   1. `autofillHints` (`AUTOFILL_HINT_USERNAME`/`PASSWORD`/`EMAIL_ADDRESS`)
 *      — sanctioned by the app/page itself, so trusted first.
 *   2. `inputType` — `TYPE_TEXT_VARIATION_PASSWORD`/`WEB_PASSWORD`/
 *      `NUMBER_VARIATION_PASSWORD` unambiguously means a password field
 *      regardless of hints.
 *   3. HTML attributes (`htmlInfo`) for WebView-hosted forms — `type`,
 *      `name`, `id`, `autocomplete` attribute values.
 *   4. `idEntry`/`hint`/`text` keyword matching as a last resort, for
 *      apps that tag neither hints nor use standard HTML forms.
 *
 * Only ever reads structure metadata (ids, hints, types) — never field
 * *values* the user may have already typed, which this service has no
 * legitimate reason to see.
 */
object StructureParser {
    private val usernameKeywords = listOf("user", "email", "login", "account", "identifier")
    private val passwordKeywords = listOf("pass", "pwd")

    fun parse(structure: AssistStructure): ParsedForm {
        var webDomain: String? = null
        val fields = mutableListOf<DetectedField>()

        for (i in 0 until structure.windowNodeCount) {
            val window = structure.getWindowNodeAt(i)
            visit(window.rootViewNode, fields) { domain -> if (webDomain == null) webDomain = domain }
        }
        return ParsedForm(webDomain = webDomain, packageName = structure.activityComponent?.packageName, fields = fields)
    }

    private fun visit(node: AssistStructure.ViewNode, out: MutableList<DetectedField>, onWebDomain: (String) -> Unit) {
        node.webDomain?.let(onWebDomain)

        val autofillId = node.autofillId
        if (autofillId != null && isTextField(node)) {
            classify(node)?.let { out.add(DetectedField(autofillId, it)) }
        }

        for (i in 0 until node.childCount) {
            visit(node.getChildAt(i), out, onWebDomain)
        }
    }

    private fun isTextField(node: AssistStructure.ViewNode): Boolean =
        node.className?.contains("EditText") == true || node.autofillType == View.AUTOFILL_TYPE_TEXT

    private fun classify(node: AssistStructure.ViewNode): FieldKind? {
        // Signal 1: explicit autofill hints, the strongest and most trustworthy signal.
        node.autofillHints?.forEach { hint ->
            when (hint) {
                View.AUTOFILL_HINT_PASSWORD -> return FieldKind.PASSWORD
                View.AUTOFILL_HINT_USERNAME, View.AUTOFILL_HINT_EMAIL_ADDRESS -> return FieldKind.USERNAME
            }
        }

        // Signal 2: input type unambiguously indicates a password field.
        val variation = node.inputType and InputType.TYPE_MASK_VARIATION
        val classField = node.inputType and InputType.TYPE_MASK_CLASS
        if (classField == InputType.TYPE_CLASS_TEXT &&
            (variation == InputType.TYPE_TEXT_VARIATION_PASSWORD ||
                variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD ||
                variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD)
        ) {
            return FieldKind.PASSWORD
        }
        if (classField == InputType.TYPE_CLASS_NUMBER && variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD) {
            return FieldKind.PASSWORD
        }
        if (classField == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS) {
            return FieldKind.USERNAME
        }

        // Signal 3: HTML form attributes, for WebView-hosted login pages.
        // Iterates by index rather than destructuring: `htmlInfo.attributes`
        // is a platform `List<android.util.Pair<String, String>>`, whose
        // Kotlin destructuring resolution is ambiguous through the SDK's
        // Java generic signature.
        val htmlAttrs = node.htmlInfo?.attributes
        for (i in 0 until (htmlAttrs?.size ?: 0)) {
            val attr = htmlAttrs!![i]
            val name = attr.first
            val value = attr.second
            if (name !in setOf("type", "name", "id", "autocomplete")) continue
            val v = value.lowercase()
            if (v.contains("password")) return FieldKind.PASSWORD
            if (passwordKeywords.any { v.contains(it) }) return FieldKind.PASSWORD
            if (usernameKeywords.any { v.contains(it) }) return FieldKind.USERNAME
        }

        // Signal 4: last resort, keyword matching on the field's own identifiers/hints.
        val haystack = listOfNotNull(node.idEntry, node.hint, node.text?.toString()).joinToString(" ").lowercase()
        if (haystack.isNotBlank()) {
            if (passwordKeywords.any { haystack.contains(it) }) return FieldKind.PASSWORD
            if (usernameKeywords.any { haystack.contains(it) }) return FieldKind.USERNAME
        }

        return null
    }
}
