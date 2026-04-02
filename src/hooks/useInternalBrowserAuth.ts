/**
 * useInternalBrowserAuth - 内置浏览器认证 Hook
 *
 * 提供使用内置浏览器进行 OAuth 认证的功能。
 */

import { useState, useCallback, useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { authApi, type CredentialInfo } from "@/lib/api/auth";

export interface AuthBrowserEvent {
  type: "UrlChanged" | "AuthCompleted" | "Closed" | "Error";
  url?: string;
  code?: string;
  message?: string;
}

export interface UseInternalBrowserAuthOptions {
  provider: string;
  onAuthCompleted?: (code?: string) => void;
  onClosed?: () => void;
  onError?: (message: string) => void;
}

export interface UseInternalBrowserAuthResult {
  // 状态
  isOpen: boolean;
  browserLabel: string | null;
  savedCredentials: CredentialInfo[];
  isLoadingCredentials: boolean;

  // 浏览器操作
  openBrowser: (
    verificationUrl: string,
    selectedUsername?: string,
  ) => Promise<void>;
  closeBrowser: () => Promise<void>;

  // 凭据操作
  loadCredentials: () => Promise<void>;
  saveCredential: (
    username: string,
    password: string,
    displayName?: string,
  ) => Promise<void>;
  deleteCredential: (username: string) => Promise<void>;
  getCredentialPassword: (username: string) => Promise<string | null>;
}

export function useInternalBrowserAuth(
  options: UseInternalBrowserAuthOptions,
): UseInternalBrowserAuthResult {
  const { provider, onAuthCompleted, onClosed, onError } = options;

  const [isOpen, setIsOpen] = useState(false);
  const [browserLabel, setBrowserLabel] = useState<string | null>(null);
  const [savedCredentials, setSavedCredentials] = useState<CredentialInfo[]>(
    [],
  );
  const [isLoadingCredentials, setIsLoadingCredentials] = useState(false);

  // 监听浏览器事件
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<AuthBrowserEvent>(
        "auth-browser-event",
        (event) => {
          const payload = event.payload;

          switch (payload.type) {
            case "AuthCompleted":
              setIsOpen(false);
              setBrowserLabel(null);
              onAuthCompleted?.(payload.code);
              break;

            case "Closed":
              setIsOpen(false);
              setBrowserLabel(null);
              onClosed?.();
              break;

            case "Error":
              onError?.(payload.message ?? "Unknown error");
              break;

            case "UrlChanged":
              // 可以用于追踪用户登录进度
              break;
          }
        },
      );
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, [onAuthCompleted, onClosed, onError]);

  // 加载已保存的凭据
  const loadCredentials = useCallback(async () => {
    setIsLoadingCredentials(true);
    try {
      const credentials = await authApi.getAuthCredentials(provider);
      setSavedCredentials(credentials);
    } catch (error) {
      console.error("Failed to load credentials:", error);
    } finally {
      setIsLoadingCredentials(false);
    }
  }, [provider]);

  // 组件挂载时加载凭据
  useEffect(() => {
    loadCredentials();
  }, [loadCredentials]);

  // 打开浏览器
  const openBrowser = useCallback(
    async (verificationUrl: string, selectedUsername?: string) => {
      try {
        let prefillUsername: string | undefined;
        let prefillPassword: string | undefined;

        // 如果选择了用户名，获取密码
        if (selectedUsername) {
          const credential = await authApi.getAuthCredential(
            provider,
            selectedUsername,
          );
          if (credential) {
            prefillUsername = credential.username;
            prefillPassword = credential.password;
            // 更新最后使用时间
            await authApi.updateCredentialLastUsed(provider, selectedUsername);
          }
        }

        const label = await authApi.openCopilotAuthBrowser(
          verificationUrl,
          prefillUsername,
          prefillPassword,
        );

        setBrowserLabel(label);
        setIsOpen(true);
      } catch (error) {
        console.error("Failed to open auth browser:", error);
        onError?.(String(error));
      }
    },
    [provider, onError],
  );

  // 关闭浏览器
  const closeBrowser = useCallback(async () => {
    if (browserLabel) {
      try {
        await authApi.closeAuthBrowser(browserLabel);
      } catch (error) {
        console.error("Failed to close auth browser:", error);
      }
    }
    setIsOpen(false);
    setBrowserLabel(null);
  }, [browserLabel]);

  // 保存凭据
  const saveCredential = useCallback(
    async (username: string, password: string, displayName?: string) => {
      try {
        await authApi.saveAuthCredential(
          provider,
          username,
          password,
          displayName,
        );
        await loadCredentials();
      } catch (error) {
        console.error("Failed to save credential:", error);
        throw error;
      }
    },
    [provider, loadCredentials],
  );

  // 删除凭据
  const deleteCredential = useCallback(
    async (username: string) => {
      try {
        await authApi.deleteAuthCredential(provider, username);
        await loadCredentials();
      } catch (error) {
        console.error("Failed to delete credential:", error);
        throw error;
      }
    },
    [provider, loadCredentials],
  );

  // 获取凭据密码
  const getCredentialPassword = useCallback(
    async (username: string): Promise<string | null> => {
      try {
        const credential = await authApi.getAuthCredential(provider, username);
        return credential?.password ?? null;
      } catch (error) {
        console.error("Failed to get credential password:", error);
        return null;
      }
    },
    [provider],
  );

  return {
    isOpen,
    browserLabel,
    savedCredentials,
    isLoadingCredentials,
    openBrowser,
    closeBrowser,
    loadCredentials,
    saveCredential,
    deleteCredential,
    getCredentialPassword,
  };
}
