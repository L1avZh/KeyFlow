package app.keyflow.mobile.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.keyflow_mobile.CredentialRecord
import uniffi.keyflow_mobile.ImportPreviewRecord
import uniffi.keyflow_mobile.MobileException
import uniffi.keyflow_mobile.MobileVault
import uniffi.keyflow_mobile.NewCredential
import uniffi.keyflow_mobile.SecurityOverviewRecord
import uniffi.keyflow_mobile.minMasterPasswordLength
import uniffi.keyflow_mobile.vaultExists

sealed interface VaultUiState {
    data object NoVaultYet : VaultUiState
    data object Locked : VaultUiState
    data class Unlocked(val credentials: List<CredentialRecord>) : VaultUiState
}

/** Every failure the UI needs to branch on distinctly, mirroring the Rust `MobileError` enum one-for-one. */
sealed class VaultOperationError(message: String) : Exception(message) {
    data object AuthenticationFailed : VaultOperationError("Incorrect master password.")
    data object CorruptVault : VaultOperationError("This vault file is corrupted or unreadable.")
    data class UnsupportedVersion(val found: UInt, val supported: UInt) :
        VaultOperationError("This vault was created by a newer version of KeyFlow.")
    data class WeakMasterPassword(val minLength: UInt) :
        VaultOperationError("Use at least $minLength characters.")
    data object CredentialNotFound : VaultOperationError("That login could not be found.")
    data object Locked : VaultOperationError("Vault is locked.")
    data class Other(val raw: String) : VaultOperationError(raw)

    companion object {
        fun from(e: MobileException): VaultOperationError = when (e) {
            is MobileException.AuthenticationFailed -> AuthenticationFailed
            is MobileException.CorruptVault -> CorruptVault
            is MobileException.UnsupportedVersion -> UnsupportedVersion(e.found, e.supported)
            is MobileException.WeakMasterPassword -> WeakMasterPassword(e.minLength)
            is MobileException.CredentialNotFound -> CredentialNotFound
            is MobileException.VaultLocked -> Locked
            else -> Other(e.message ?: e.toString())
        }
    }
}

/**
 * The Android equivalent of the desktop app's `Mutex<Option<Vault>>` app
 * state (`apps/desktop/src-tauri/src/state.rs`). "Locked" is represented
 * by holding no [MobileVault] instance at all — closing it (via
 * `AutoCloseable`) drops the Rust-side `Arc<Mutex<Vault>>`, which
 * zeroizes the derived key immediately rather than waiting on GC.
 *
 * Every method here does real, blocking file I/O and Argon2id/AES-GCM
 * work via the JNA call into `keyflow-mobile` — always dispatched onto
 * [Dispatchers.IO], never left on the caller's thread, since
 * `keyflow-core` has no async runtime of its own (confirmed zero
 * tokio/async usage in the crate) and would otherwise block whatever
 * thread called in, including the UI thread if a caller forgot.
 */
class VaultRepository(private val vaultFilePath: String) {
    // `@Volatile` for cross-thread visibility, `vaultFieldLock` for
    // atomicity of the read-then-use and check-then-set sequences below.
    // This matters here specifically because `lock()` can run from a
    // background auto-lock coroutine (`noteUserActivity`, dispatched on
    // Dispatchers.Default) concurrently with `unlock()`/`createVault()`
    // running on Dispatchers.IO from a UI action — without this, a plain
    // `var` could let one thread observe a stale or torn value of the
    // other's write. The actual Rust FFI call itself always happens
    // *outside* the lock (suspending inside a `synchronized` block is
    // illegal in Kotlin coroutines) — only the field's own read/write is
    // guarded.
    @Volatile
    private var vault: MobileVault? = null
    private val vaultFieldLock = Any()
    private var autoLockJob: Job? = null

    private val _state = MutableStateFlow<VaultUiState>(
        if (vaultExists(vaultFilePath)) VaultUiState.Locked else VaultUiState.NoVaultYet
    )
    val state: StateFlow<VaultUiState> = _state

    val minMasterPasswordLength: UInt get() = minMasterPasswordLength()

    suspend fun createVault(masterPassword: String): Result<Unit> = runCatching {
        val newVault = withContext(Dispatchers.IO) { MobileVault.create(vaultFilePath, masterPassword) }
        synchronized(vaultFieldLock) { vault = newVault }
        refresh()
    }.mapFailure()

    suspend fun unlock(masterPassword: String): Result<Unit> = runCatching {
        val newVault = withContext(Dispatchers.IO) { MobileVault.unlock(vaultFilePath, masterPassword) }
        synchronized(vaultFieldLock) { vault = newVault }
        refresh()
    }.mapFailure()

    /** Drops the vault handle (zeroizing the key) and cancels any pending auto-lock timer. */
    fun lock() {
        autoLockJob?.cancel()
        val old = synchronized(vaultFieldLock) {
            val v = vault
            vault = null
            v
        }
        old?.close()
        _state.update { VaultUiState.Locked }
    }

    /** Restarts the countdown to [lock]; call on every user interaction while unlocked. */
    fun noteUserActivity(autoLockMinutes: Int, scope: CoroutineScope) {
        autoLockJob?.cancel()
        if (autoLockMinutes <= 0 || vault == null) return
        autoLockJob = scope.launch(Dispatchers.Default) {
            delay(autoLockMinutes * 60_000L)
            lock()
        }
    }

    private suspend fun refresh() {
        val v = synchronized(vaultFieldLock) { vault } ?: return
        val creds = withContext(Dispatchers.IO) { v.listCredentials() }
        _state.update { VaultUiState.Unlocked(creds) }
    }

    suspend fun addCredential(input: NewCredential): Result<String> =
        withVault { it.addCredential(input) }.also { refreshIfUnlocked() }

    suspend fun updateCredential(credential: CredentialRecord): Result<Unit> =
        withVault { it.updateCredential(credential) }.also { refreshIfUnlocked() }

    suspend fun deleteCredential(id: String): Result<Unit> =
        withVault { it.deleteCredential(id) }.also { refreshIfUnlocked() }

    suspend fun touchCredentialUsed(id: String): Result<Unit> =
        withVault { it.touchCredentialUsed(id) }.also { refreshIfUnlocked() }

    suspend fun changeMasterPassword(newPassword: String): Result<Unit> = withVault { it.changeMasterPassword(newPassword) }

    suspend fun securityOverview(): Result<SecurityOverviewRecord> = withVault { it.securityOverview() }

    suspend fun weakCredentials(): Result<List<CredentialRecord>> = withVault { it.weakCredentials() }

    suspend fun oldCredentials(olderThanDays: Long): Result<List<CredentialRecord>> = withVault { it.oldCredentials(olderThanDays) }

    suspend fun reusedCredentialGroups(): Result<List<List<CredentialRecord>>> = withVault { it.reusedCredentialGroups() }

    suspend fun duplicateCredentialGroups(): Result<List<List<CredentialRecord>>> = withVault { it.duplicateCredentialGroups() }

    suspend fun findAutofillMatches(url: String) = withVault { it.findAutofillMatches(url) }

    suspend fun exportJsonPlaintext(): Result<String> = withVault { it.exportJsonPlaintext() }

    suspend fun previewCsvImport(csvText: String): Result<ImportPreviewRecord> = withVault { it.previewCsvImport(csvText) }

    suspend fun commitImport(credentials: List<NewCredential>): Result<Unit> =
        withVault { it.commitImport(credentials) }.also { refreshIfUnlocked() }

    private suspend fun refreshIfUnlocked() {
        if (synchronized(vaultFieldLock) { vault } != null) refresh()
    }

    private suspend fun <T> withVault(block: (MobileVault) -> T): Result<T> {
        val v = synchronized(vaultFieldLock) { vault } ?: return Result.failure(VaultOperationError.Locked)
        return runCatching { withContext(Dispatchers.IO) { block(v) } }.mapFailure()
    }

    private fun <T> Result<T>.mapFailure(): Result<T> {
        val ex = exceptionOrNull() ?: return this
        return when {
            ex is MobileException -> Result.failure(VaultOperationError.from(ex))
            // Narrow but real race: `lock()` (e.g. the auto-lock timer)
            // can close the vault handle between `withVault`'s read of
            // `vault` and the FFI call actually running on
            // Dispatchers.IO — UniFFI's generated object throws this
            // specific message in that case (see its `callWithPointer`).
            // Surfacing it as the same `Locked` error the caller already
            // has to handle is correct: the vault genuinely is locked now.
            ex is IllegalStateException && ex.message?.contains("already been destroyed") == true ->
                Result.failure(VaultOperationError.Locked)
            else -> this
        }
    }
}
