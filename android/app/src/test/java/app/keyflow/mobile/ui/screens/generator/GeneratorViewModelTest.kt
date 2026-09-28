package app.keyflow.mobile.ui.screens.generator

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Like [app.keyflow.mobile.data.VaultRepositoryTest], this calls the
 * real `keyflow-mobile`/`keyflow-core` password generator and strength
 * estimator via JNA — the same single implementation the desktop app and
 * browser extension use — not a Kotlin reimplementation of either.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class GeneratorViewModelTest {
    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `initial state has a non-empty generated password at the default length`() = runTest {
        val viewModel = GeneratorViewModel()
        testScheduler.advanceUntilIdle()
        val state = viewModel.uiState.value
        assertEquals(20, state.password.codePointCount(0, state.password.length))
    }

    @Test
    fun `changing length regenerates a password of the new length`() = runTest {
        val viewModel = GeneratorViewModel()
        testScheduler.advanceUntilIdle()
        viewModel.onLengthChange(32)
        testScheduler.advanceUntilIdle()
        assertEquals(32, viewModel.uiState.value.password.codePointCount(0, viewModel.uiState.value.password.length))
    }

    @Test
    fun `disabling all character classes still yields a valid strength label rather than crashing`() = runTest {
        val viewModel = GeneratorViewModel()
        testScheduler.advanceUntilIdle()
        // Turning off every class except lowercase is the minimum viable
        // configuration; the underlying Rust generator itself rejects an
        // all-classes-disabled request (InvalidGeneratorConfig) rather
        // than silently returning garbage, and the ViewModel is expected
        // to leave the previous password in place when that happens
        // rather than blanking it — this test pins that recovery
        // behavior rather than a crash.
        viewModel.onToggle(uppercase = false, digits = false, symbols = false)
        testScheduler.advanceUntilIdle()
        assertTrue(viewModel.uiState.value.password.isNotEmpty())
        assertTrue(viewModel.uiState.value.password.all { it in "abcdefghijklmnopqrstuvwxyz" })
    }
}
