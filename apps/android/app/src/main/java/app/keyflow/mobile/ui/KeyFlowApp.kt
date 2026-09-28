package app.keyflow.mobile.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.fragment.app.FragmentActivity
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import app.keyflow.mobile.biometric.BiometricVaultUnlocker
import app.keyflow.mobile.data.AppPreferences
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.data.VaultUiState
import app.keyflow.mobile.ui.navigation.Routes
import app.keyflow.mobile.ui.screens.createvault.CreateVaultScreen
import app.keyflow.mobile.ui.screens.detail.CredentialDetailScreen
import app.keyflow.mobile.ui.screens.generator.GeneratorScreen
import app.keyflow.mobile.ui.screens.onboarding.OnboardingScreen
import app.keyflow.mobile.ui.screens.security.SecurityScreen
import app.keyflow.mobile.ui.screens.settings.SettingsScreen
import app.keyflow.mobile.ui.screens.unlock.UnlockScreen
import app.keyflow.mobile.ui.screens.vaultlist.VaultListScreen
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

@Composable
fun KeyFlowApp(
    vaultRepository: VaultRepository,
    preferences: AppPreferences,
    biometricUnlocker: BiometricVaultUnlocker,
    activity: FragmentActivity,
) {
    val navController = rememberNavController()
    val scope = rememberCoroutineScope()
    val vaultState by vaultRepository.state.collectAsState()
    var onboardingComplete by remember { mutableStateOf<Boolean?>(null) }

    LaunchedEffect(Unit) {
        onboardingComplete = preferences.onboardingComplete.first()
    }

    val knownOnboardingComplete = onboardingComplete ?: return

    val startDestination = when {
        !knownOnboardingComplete -> Routes.ONBOARDING
        vaultState is VaultUiState.NoVaultYet -> Routes.CREATE_VAULT
        else -> Routes.UNLOCK
    }

    NavHost(navController = navController, startDestination = startDestination) {
        composable(Routes.ONBOARDING) {
            OnboardingScreen(
                onDone = {
                    scope.launch {
                        preferences.setOnboardingComplete(true)
                        val next = if (vaultState is VaultUiState.NoVaultYet) Routes.CREATE_VAULT else Routes.UNLOCK
                        navController.navigate(next) { popUpTo(0) }
                    }
                },
            )
        }
        composable(Routes.CREATE_VAULT) {
            CreateVaultScreen(
                vaultRepository = vaultRepository,
                onCreated = { navController.navigate(Routes.VAULT_LIST) { popUpTo(0) } },
            )
        }
        composable(Routes.UNLOCK) {
            UnlockScreen(
                vaultRepository = vaultRepository,
                biometricUnlocker = biometricUnlocker,
                activity = activity,
                onUnlocked = { navController.navigate(Routes.VAULT_LIST) { popUpTo(0) } },
            )
        }
        composable(Routes.VAULT_LIST) {
            VaultListScreen(
                vaultRepository = vaultRepository,
                onOpenCredential = { id -> navController.navigate(Routes.credentialDetail(id)) },
                onAddCredential = { navController.navigate(Routes.CREDENTIAL_NEW) },
                onOpenGenerator = { navController.navigate(Routes.GENERATOR) },
                onOpenSecurity = { navController.navigate(Routes.SECURITY) },
                onOpenSettings = { navController.navigate(Routes.SETTINGS) },
            )
        }
        composable(Routes.CREDENTIAL_NEW) {
            CredentialDetailScreen(vaultRepository = vaultRepository, credentialId = null, onDone = { navController.popBackStack() })
        }
        composable(
            route = Routes.CREDENTIAL_DETAIL,
            arguments = listOf(navArgument("credentialId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val id = backStackEntry.arguments?.getString("credentialId")
            CredentialDetailScreen(vaultRepository = vaultRepository, credentialId = id, onDone = { navController.popBackStack() })
        }
        composable(Routes.GENERATOR) {
            GeneratorScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.SECURITY) {
            SecurityScreen(vaultRepository = vaultRepository, onBack = { navController.popBackStack() }, onOpenCredential = { id -> navController.navigate(Routes.credentialDetail(id)) })
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                vaultRepository = vaultRepository,
                preferences = preferences,
                biometricUnlocker = biometricUnlocker,
                activity = activity,
                onBack = { navController.popBackStack() },
                onLocked = { navController.navigate(Routes.UNLOCK) { popUpTo(0) } },
            )
        }
    }

    LaunchedEffect(vaultState) {
        if (vaultState is VaultUiState.Locked && navController.currentBackStackEntry?.destination?.route != Routes.UNLOCK) {
            navController.navigate(Routes.UNLOCK) { popUpTo(0) }
        }
    }
}
