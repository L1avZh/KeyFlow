package app.keyflow.mobile.ui.navigation

object Routes {
    const val ONBOARDING = "onboarding"
    const val CREATE_VAULT = "create_vault"
    const val UNLOCK = "unlock"
    const val VAULT_LIST = "vault_list"
    const val CREDENTIAL_DETAIL = "credential_detail/{credentialId}"
    const val CREDENTIAL_NEW = "credential_new"
    const val GENERATOR = "generator"
    const val SECURITY = "security"
    const val SETTINGS = "settings"

    fun credentialDetail(id: String) = "credential_detail/$id"
}
