package app.keyflow.mobile.autofill

import android.app.PendingIntent
import android.content.Intent
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.Dataset
import android.service.autofill.FillCallback
import android.service.autofill.FillRequest
import android.service.autofill.FillResponse
import android.service.autofill.SaveCallback
import android.service.autofill.SaveRequest
import android.view.autofill.AutofillValue
import android.widget.RemoteViews
import app.keyflow.mobile.KeyFlowApplication
import app.keyflow.mobile.R
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.data.VaultUiState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.keyflow_mobile.AutofillMatchRecord

/**
 * KeyFlow's Android Autofill Framework provider. Deliberately built on
 * the platform's own Autofill Framework rather than an Accessibility
 * Service — Accessibility gives blanket access to every event and piece
 * of text across every app the user runs, which is far more privilege
 * than filling a form needs and a well-known way password managers have
 * historically over-reached; the Autofill Framework instead gives this
 * service only the specific view-tree structure of the one screen the
 * user is filling, mediated by the OS.
 *
 * Runs in this app's own process, so it shares the same in-memory
 * [VaultRepository] singleton the main activity uses: if the vault is
 * already unlocked there, autofill "just works" with no extra prompt; if
 * it's locked, the response defers to [AutofillAuthActivity] to unlock
 * first — this service itself never asks for or stores a master
 * password.
 */
class KeyFlowAutofillService : AutofillService() {
    private val job = Job()
    private val scope = CoroutineScope(Dispatchers.Default + job)

    override fun onDestroy() {
        super.onDestroy()
        job.cancel()
    }

    override fun onFillRequest(request: FillRequest, cancellationSignal: CancellationSignal, callback: FillCallback) {
        val structure = request.fillContexts.lastOrNull()?.structure
        if (structure == null) {
            callback.onSuccess(null)
            return
        }
        val parsed = StructureParser.parse(structure)
        if (parsed.fields.isEmpty()) {
            callback.onSuccess(null)
            return
        }

        val origin = resolveOrigin(parsed)
        if (origin == null) {
            callback.onSuccess(null)
            return
        }

        val repository = KeyFlowApplication.from(this).vaultRepository
        val currentState = repository.state.value

        if (currentState is VaultUiState.Unlocked) {
            scope.launch {
                val matches = repository.findAutofillMatches(origin).getOrNull().orEmpty()
                val response = buildUnlockedResponse(parsed, matches)
                callback.onSuccess(response)
            }
        } else {
            callback.onSuccess(buildLockedResponse(parsed, origin))
        }
    }

    private fun resolveOrigin(parsed: ParsedForm): String? {
        parsed.webDomain?.let { return "https://$it" }
        // No verified web domain (a native app's own login screen, not a
        // WebView) — there is no cryptographically verified link between
        // an Android package name and a saved credential's URL without
        // that app publishing Digital Asset Links, which KeyFlow does not
        // currently check (see ROADMAP.md). As a best-effort heuristic
        // only — the same one several mainstream password managers use —
        // reverse the package's dot-separated segments into a candidate
        // domain (`com.github.android` -> `android.github.com`) and run
        // it through the exact same phishing-resistant matching engine
        // used for real domains, so it still only ever offers an
        // exact/subdomain match, never a loose guess.
        val pkg = parsed.packageName ?: return null
        val segments = pkg.split('.')
        if (segments.size < 2) return null
        return "https://" + segments.reversed().joinToString(".")
    }

    private fun buildUnlockedResponse(parsed: ParsedForm, matches: List<AutofillMatchRecord>): FillResponse? {
        if (matches.isEmpty()) return null
        val responseBuilder = FillResponse.Builder()
        for (match in matches) {
            val datasetBuilder = Dataset.Builder()
            var any = false
            for (field in parsed.fields) {
                val value = when (field.kind) {
                    FieldKind.USERNAME -> match.credential.username
                    FieldKind.PASSWORD -> match.credential.password
                }
                if (value.isEmpty()) continue
                datasetBuilder.setValue(field.autofillId, AutofillValue.forText(value), presentation(match.credential.name))
                any = true
            }
            if (any) responseBuilder.addDataset(datasetBuilder.build())
        }
        return try {
            responseBuilder.build()
        } catch (e: Exception) {
            // FillResponse.Builder.build() throws if no dataset ended up
            // being added (e.g. every match had an empty value for every
            // detected field) — a real, if rare, empty-result case, not
            // a bug to hide.
            null
        }
    }

    private fun buildLockedResponse(parsed: ParsedForm, origin: String): FillResponse {
        val autofillIds = parsed.fields.map { it.autofillId }.toTypedArray()
        val intent = AutofillAuthActivity.intent(this, origin, parsed.fields)
        val pendingIntent = PendingIntent.getActivity(
            this,
            origin.hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return FillResponse.Builder()
            .setAuthentication(autofillIds, pendingIntent.intentSender, presentation(getString(R.string.autofill_unlock_prompt)))
            .build()
    }

    private fun presentation(label: String): RemoteViews =
        RemoteViews(packageName, android.R.layout.simple_list_item_1).apply {
            setTextViewText(android.R.id.text1, label)
        }

    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        // Saving a *new* credential captured from another app's form is
        // deliberately not implemented yet: doing it safely needs the
        // same "review before saving" step the CSV importer already
        // gives the desktop app (duplicate/weak-password warnings)
        // rather than silently writing whatever the OS captured — see
        // ROADMAP.md. Declining (rather than half-implementing silent
        // writes) is the safer choice for a v1.
        callback.onFailure("Saving new logins from other apps isn't supported yet — add it from KeyFlow directly.")
    }
}
