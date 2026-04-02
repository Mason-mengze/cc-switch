import { invoke } from "@tauri-apps/api/core";

export type ManagedAuthProvider = "github_copilot";

export interface ManagedAuthAccount {
  id: string;
  provider: ManagedAuthProvider;
  login: string;
  avatar_url: string | null;
  authenticated_at: number;
  is_default: boolean;
}

export interface ManagedAuthStatus {
  provider: ManagedAuthProvider;
  authenticated: boolean;
  default_account_id: string | null;
  migration_error?: string | null;
  accounts: ManagedAuthAccount[];
}

export interface ManagedAuthDeviceCodeResponse {
  provider: ManagedAuthProvider;
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

// ===== 内置浏览器认证 =====

export interface AuthBrowserConfig {
  url: string;
  title: string;
  provider: string;
  success_url_pattern?: string;
  enable_autofill: boolean;
  prefill_username?: string;
  prefill_password?: string;
}

export interface CredentialInfo {
  username: string;
  display_name?: string;
  created_at: string;
  last_used_at?: string;
}

export interface DecryptedCredential {
  username: string;
  password: string;
  display_name?: string;
}

export async function authStartLogin(
  authProvider: ManagedAuthProvider,
): Promise<ManagedAuthDeviceCodeResponse> {
  return invoke<ManagedAuthDeviceCodeResponse>("auth_start_login", {
    authProvider,
  });
}

export async function authPollForAccount(
  authProvider: ManagedAuthProvider,
  deviceCode: string,
): Promise<ManagedAuthAccount | null> {
  return invoke<ManagedAuthAccount | null>("auth_poll_for_account", {
    authProvider,
    deviceCode,
  });
}

export async function authListAccounts(
  authProvider: ManagedAuthProvider,
): Promise<ManagedAuthAccount[]> {
  return invoke<ManagedAuthAccount[]>("auth_list_accounts", {
    authProvider,
  });
}

export async function authGetStatus(
  authProvider: ManagedAuthProvider,
): Promise<ManagedAuthStatus> {
  return invoke<ManagedAuthStatus>("auth_get_status", {
    authProvider,
  });
}

export async function authRemoveAccount(
  authProvider: ManagedAuthProvider,
  accountId: string,
): Promise<void> {
  return invoke("auth_remove_account", {
    authProvider,
    accountId,
  });
}

export async function authSetDefaultAccount(
  authProvider: ManagedAuthProvider,
  accountId: string,
): Promise<void> {
  return invoke("auth_set_default_account", {
    authProvider,
    accountId,
  });
}

export async function authLogout(
  authProvider: ManagedAuthProvider,
): Promise<void> {
  return invoke("auth_logout", {
    authProvider,
  });
}

// ===== 内置浏览器管理 =====

export async function openAuthBrowser(
  config: AuthBrowserConfig,
): Promise<string> {
  return invoke<string>("open_auth_browser", { config });
}

export async function openCopilotAuthBrowser(
  verificationUrl: string,
  prefillUsername?: string,
  prefillPassword?: string,
): Promise<string> {
  return invoke<string>("open_copilot_auth_browser", {
    verificationUrl,
    prefillUsername,
    prefillPassword,
  });
}

export async function closeAuthBrowser(label: string): Promise<void> {
  return invoke("close_auth_browser", { label });
}

export async function closeAuthBrowserForProvider(
  provider: string,
): Promise<void> {
  return invoke("close_auth_browser_for_provider", { provider });
}

export async function getActiveAuthBrowsers(): Promise<string[]> {
  return invoke<string[]>("get_active_auth_browsers");
}

// ===== 凭据管理 =====

export async function saveAuthCredential(
  provider: string,
  username: string,
  password: string,
  displayName?: string,
): Promise<void> {
  return invoke("save_auth_credential", {
    provider,
    username,
    password,
    displayName,
  });
}

export async function getAuthCredentials(
  provider: string,
): Promise<CredentialInfo[]> {
  return invoke<CredentialInfo[]>("get_auth_credentials", { provider });
}

export async function getAuthCredential(
  provider: string,
  username: string,
): Promise<DecryptedCredential | null> {
  return invoke<DecryptedCredential | null>("get_auth_credential", {
    provider,
    username,
  });
}

export async function updateCredentialLastUsed(
  provider: string,
  username: string,
): Promise<void> {
  return invoke("update_credential_last_used", { provider, username });
}

export async function deleteAuthCredential(
  provider: string,
  username: string,
): Promise<void> {
  return invoke("delete_auth_credential", { provider, username });
}

export async function deleteAllAuthCredentials(provider: string): Promise<void> {
  return invoke("delete_all_auth_credentials", { provider });
}

export const authApi = {
  authStartLogin,
  authPollForAccount,
  authListAccounts,
  authGetStatus,
  authRemoveAccount,
  authSetDefaultAccount,
  authLogout,
  // Internal browser
  openAuthBrowser,
  openCopilotAuthBrowser,
  closeAuthBrowser,
  closeAuthBrowserForProvider,
  getActiveAuthBrowsers,
  // Credentials
  saveAuthCredential,
  getAuthCredentials,
  getAuthCredential,
  updateCredentialLastUsed,
  deleteAuthCredential,
  deleteAllAuthCredentials,
};
