package app.keyflow.mobile.data

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.keyflow_mobile.NewCredential
import java.io.File
import kotlin.io.path.createTempDirectory

/**
 * Exercises [VaultRepository] against the *real*, host-compiled
 * `keyflow-mobile`/`keyflow-core` Rust library via JNA (see
 * `jna.library.path` wiring in app/build.gradle.kts) — not a mock or
 * fake. These tests genuinely create an AES-256-GCM/Argon2id-encrypted
 * vault file on disk and read it back, the same engine the desktop app
 * and browser extension use. If this file's tests pass, the Kotlin
 * repository layer is correctly calling into the real vault engine, not
 * just correctly calling into some in-memory fake of it.
 */
class VaultRepositoryTest {
    private lateinit var vaultPath: String

    @Before
    fun setUp() {
        val dir = createTempDirectory("keyflow-repo-test").toFile()
        vaultPath = File(dir, "vault.keyflow").absolutePath
    }

    @Test
    fun `starts in NoVaultYet when no vault file exists`() {
        val repo = VaultRepository(vaultPath)
        assertTrue(repo.state.value is VaultUiState.NoVaultYet)
    }

    @Test
    fun `create then lock then unlock round trips a credential through the real vault`() = runTest {
        val repo = VaultRepository(vaultPath)
        repo.createVault("correct horse battery staple").getOrThrow()

        val addResult = repo.addCredential(
            NewCredential(name = "GitHub", url = "https://github.com", username = "me", password = "hunter2", notes = "", tags = emptyList(), favorite = false),
        )
        assertTrue(addResult.isSuccess)

        repo.lock()
        assertTrue(repo.state.value is VaultUiState.Locked)

        repo.unlock("correct horse battery staple").getOrThrow()
        val state = repo.state.value as VaultUiState.Unlocked
        assertEquals(1, state.credentials.size)
        assertEquals("GitHub", state.credentials[0].name)
        assertEquals("hunter2", state.credentials[0].password)
    }

    @Test
    fun `unlock with wrong password maps to AuthenticationFailed`() = runTest {
        val repo = VaultRepository(vaultPath)
        repo.createVault("correct horse battery staple").getOrThrow()
        repo.lock()

        val result = repo.unlock("totally wrong password")
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull() is VaultOperationError.AuthenticationFailed)
    }

    @Test
    fun `create with a too-short password maps to WeakMasterPassword with the real min length`() = runTest {
        val repo = VaultRepository(vaultPath)
        val result = repo.createVault("short")
        val error = result.exceptionOrNull()
        assertTrue(error is VaultOperationError.WeakMasterPassword)
        assertEquals(repo.minMasterPasswordLength, (error as VaultOperationError.WeakMasterPassword).minLength)
    }

    @Test
    fun `operating on a locked vault fails with Locked rather than throwing`() = runTest {
        val repo = VaultRepository(vaultPath)
        // Never created/unlocked — repo.vault is null.
        val result = repo.addCredential(
            NewCredential(name = "x", url = "", username = "", password = "", notes = "", tags = emptyList(), favorite = false),
        )
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull() is VaultOperationError.Locked)
    }

    @Test
    fun `deleting a credential removes it and refreshes state`() = runTest {
        val repo = VaultRepository(vaultPath)
        repo.createVault("correct horse battery staple").getOrThrow()
        val id = repo.addCredential(
            NewCredential(name = "GitHub", url = "https://github.com", username = "me", password = "pw", notes = "", tags = emptyList(), favorite = false),
        ).getOrThrow()

        repo.deleteCredential(id).getOrThrow()
        val state = repo.state.value as VaultUiState.Unlocked
        assertTrue(state.credentials.isEmpty())
    }

    @Test
    fun `security overview reflects a real weak password computed by the Rust engine`() = runTest {
        val repo = VaultRepository(vaultPath)
        repo.createVault("correct horse battery staple").getOrThrow()
        repo.addCredential(
            NewCredential(name = "Weak", url = "https://example.com", username = "me", password = "a", notes = "", tags = emptyList(), favorite = false),
        ).getOrThrow()

        val overview = repo.securityOverview().getOrThrow()
        assertEquals(1, overview.total.toInt())
        assertEquals(1, overview.weak.toInt())
    }

    @Test
    fun `lock zeroes out the in-memory vault handle so re-locking is a harmless no-op`() = runTest {
        val repo = VaultRepository(vaultPath)
        repo.createVault("correct horse battery staple").getOrThrow()
        repo.lock()
        repo.lock() // must not throw
        assertTrue(repo.state.value is VaultUiState.Locked)
    }
}
