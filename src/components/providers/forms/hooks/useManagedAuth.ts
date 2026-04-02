import { useState, useCallback, useRef, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { authApi, settingsApi } from "@/lib/api";
import type {
  ManagedAuthProvider,
  ManagedAuthStatus,
  ManagedAuthDeviceCodeResponse,
  CredentialInfo,
} from "@/lib/api";
import { useSettingsQuery } from "@/lib/query";

type PollingState = "idle" | "polling" | "success" | "error";

interface AuthBrowserEvent {
  type: "UrlChanged" | "AuthCompleted" | "Closed" | "Error";
  url?: string;
  code?: string;
  message?: string;
}

export function useManagedAuth(authProvider: ManagedAuthProvider) {
  const queryClient = useQueryClient();
  const queryKey = ["managed-auth-status", authProvider];

  const [pollingState, setPollingState] = useState<PollingState>("idle");
  const [deviceCode, setDeviceCode] =
    useState<ManagedAuthDeviceCodeResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [savedCredentials, setSavedCredentials] = useState<CredentialInfo[]>(
    [],
  );
  const [isInternalBrowserOpen, setIsInternalBrowserOpen] = useState(false);

  const pollingIntervalRef = useRef<ReturnType<typeof setInterval> | null>(
    null,
  );
  const pollingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const browserLabelRef = useRef<string | null>(null);

  // 获取设置
  const { data: settings } = useSettingsQuery();
  const useInternalBrowser = settings?.useInternalBrowser ?? true;

  const {
    data: authStatus,
    isLoading: isLoadingStatus,
    refetch: refetchStatus,
  } = useQuery<ManagedAuthStatus>({
    queryKey,
    queryFn: () => authApi.authGetStatus(authProvider),
    staleTime: 30000,
  });

  // 加载已保存的凭据
  const loadCredentials = useCallback(async () => {
    try {
      const credentials = await authApi.getAuthCredentials(authProvider);
      setSavedCredentials(credentials);
    } catch (e) {
      console.debug("[ManagedAuth] Failed to load credentials:", e);
    }
  }, [authProvider]);

  // 组件挂载时加载凭据
  useEffect(() => {
    if (useInternalBrowser) {
      loadCredentials();
    }
  }, [useInternalBrowser, loadCredentials]);

  const stopPolling = useCallback(() => {
    if (pollingIntervalRef.current) {
      clearInterval(pollingIntervalRef.current);
      pollingIntervalRef.current = null;
    }
    if (pollingTimeoutRef.current) {
      clearTimeout(pollingTimeoutRef.current);
      pollingTimeoutRef.current = null;
    }
  }, []);

  // 监听内置浏览器事件
  useEffect(() => {
    if (!useInternalBrowser) return;

    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<AuthBrowserEvent>(
        "auth-browser-event",
        async (event) => {
          const payload = event.payload;

          switch (payload.type) {
            case "AuthCompleted":
              setIsInternalBrowserOpen(false);
              browserLabelRef.current = null;
              // 授权完成后继续轮询以获取账号信息
              break;

            case "Closed":
              setIsInternalBrowserOpen(false);
              browserLabelRef.current = null;
              break;

            case "Error":
              setError(payload.message ?? "Browser error");
              break;
          }
        },
      );
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, [useInternalBrowser]);

  useEffect(() => {
    return () => {
      stopPolling();
    };
  }, [stopPolling]);

  // 使用内置浏览器启动登录
  const startInternalBrowserAuth = useCallback(
    async (
      response: ManagedAuthDeviceCodeResponse,
      selectedUsername?: string,
    ) => {
      try {
        let prefillUsername: string | undefined;
        let prefillPassword: string | undefined;

        // 如果选择了用户名，获取密码
        if (selectedUsername) {
          const credential = await authApi.getAuthCredential(
            authProvider,
            selectedUsername,
          );
          if (credential) {
            prefillUsername = credential.username;
            prefillPassword = credential.password;
            await authApi.updateCredentialLastUsed(
              authProvider,
              selectedUsername,
            );
          }
        }

        const label = await authApi.openCopilotAuthBrowser(
          response.verification_uri,
          prefillUsername,
          prefillPassword,
        );

        browserLabelRef.current = label;
        setIsInternalBrowserOpen(true);
      } catch (e) {
        console.error("[ManagedAuth] Failed to open internal browser:", e);
        // 回退到默认浏览器
        try {
          await settingsApi.openExternal(response.verification_uri);
        } catch (fallbackError) {
          console.error("[ManagedAuth] Fallback to external browser failed:", fallbackError);
        }
      }
    },
    [authProvider],
  );

  const startLoginMutation = useMutation({
    mutationFn: () => authApi.authStartLogin(authProvider),
    onSuccess: async (response) => {
      setDeviceCode(response);
      setPollingState("polling");
      setError(null);

      try {
        await navigator.clipboard.writeText(response.user_code);
      } catch (e) {
        console.debug("[ManagedAuth] Failed to copy user code:", e);
      }

      // 根据设置决定使用哪种浏览器
      if (useInternalBrowser) {
        await startInternalBrowserAuth(response);
      } else {
        try {
          await settingsApi.openExternal(response.verification_uri);
        } catch (e) {
          console.debug("[ManagedAuth] Failed to open browser:", e);
        }
      }

      // Add a small buffer on top of GitHub's suggested interval to avoid
      // hitting slow_down responses too aggressively during device polling.
      const interval = Math.max((response.interval || 5) + 3, 8) * 1000;
      const expiresAt = Date.now() + response.expires_in * 1000;

      const pollOnce = async () => {
        if (Date.now() > expiresAt) {
          stopPolling();
          setPollingState("error");
          setError("Device code expired. Please try again.");
          // 关闭内置浏览器
          if (browserLabelRef.current) {
            try {
              await authApi.closeAuthBrowser(browserLabelRef.current);
            } catch (e) {
              console.debug("[ManagedAuth] Failed to close browser:", e);
            }
            browserLabelRef.current = null;
            setIsInternalBrowserOpen(false);
          }
          return;
        }

        try {
          const newAccount = await authApi.authPollForAccount(
            authProvider,
            response.device_code,
          );
          if (newAccount) {
            stopPolling();
            setPollingState("success");
            await refetchStatus();
            await queryClient.invalidateQueries({ queryKey });
            setPollingState("idle");
            setDeviceCode(null);
            // 关闭内置浏览器（如果还开着）
            if (browserLabelRef.current) {
              try {
                await authApi.closeAuthBrowser(browserLabelRef.current);
              } catch (e) {
                console.debug("[ManagedAuth] Failed to close browser:", e);
              }
              browserLabelRef.current = null;
              setIsInternalBrowserOpen(false);
            }
          }
        } catch (e) {
          const errorMessage = e instanceof Error ? e.message : String(e);
          if (
            !errorMessage.includes("pending") &&
            !errorMessage.includes("slow_down")
          ) {
            stopPolling();
            setPollingState("error");
            setError(errorMessage);
          }
        }
      };

      void pollOnce();
      pollingIntervalRef.current = setInterval(pollOnce, interval);
      pollingTimeoutRef.current = setTimeout(() => {
        stopPolling();
        setPollingState("error");
        setError("Device code expired. Please try again.");
      }, response.expires_in * 1000);
    },
    onError: (e) => {
      setPollingState("error");
      setError(e instanceof Error ? e.message : String(e));
    },
  });

  const logoutMutation = useMutation({
    mutationFn: () => authApi.authLogout(authProvider),
    onSuccess: async () => {
      setPollingState("idle");
      setDeviceCode(null);
      setError(null);
      queryClient.setQueryData(queryKey, {
        provider: authProvider,
        authenticated: false,
        default_account_id: null,
        accounts: [],
      });
      await queryClient.invalidateQueries({ queryKey });
    },
    onError: async (e) => {
      console.error("[ManagedAuth] Failed to logout:", e);
      setError(e instanceof Error ? e.message : String(e));
      await refetchStatus();
    },
  });

  const removeAccountMutation = useMutation({
    mutationFn: (accountId: string) =>
      authApi.authRemoveAccount(authProvider, accountId),
    onSuccess: async () => {
      setPollingState("idle");
      setDeviceCode(null);
      setError(null);
      await refetchStatus();
      await queryClient.invalidateQueries({ queryKey });
    },
    onError: (e) => {
      console.error("[ManagedAuth] Failed to remove account:", e);
      setError(e instanceof Error ? e.message : String(e));
    },
  });

  const setDefaultAccountMutation = useMutation({
    mutationFn: (accountId: string) =>
      authApi.authSetDefaultAccount(authProvider, accountId),
    onSuccess: async () => {
      await refetchStatus();
      await queryClient.invalidateQueries({ queryKey });
    },
    onError: (e) => {
      console.error("[ManagedAuth] Failed to set default account:", e);
      setError(e instanceof Error ? e.message : String(e));
    },
  });

  const startAuth = useCallback(() => {
    setPollingState("idle");
    setDeviceCode(null);
    setError(null);
    stopPolling();
    startLoginMutation.mutate();
  }, [startLoginMutation, stopPolling]);

  // 使用指定凭据启动认证（内置浏览器模式）
  const startAuthWithCredential = useCallback(
    async (selectedUsername?: string) => {
      setPollingState("idle");
      setDeviceCode(null);
      setError(null);
      stopPolling();

      try {
        const response = await authApi.authStartLogin(authProvider);
        setDeviceCode(response);
        setPollingState("polling");
        setError(null);

        try {
          await navigator.clipboard.writeText(response.user_code);
        } catch (e) {
          console.debug("[ManagedAuth] Failed to copy user code:", e);
        }

        // 使用内置浏览器并传入凭据
        await startInternalBrowserAuth(response, selectedUsername);

        // 开始轮询
        const interval = Math.max((response.interval || 5) + 3, 8) * 1000;
        const expiresAt = Date.now() + response.expires_in * 1000;

        const pollOnce = async () => {
          if (Date.now() > expiresAt) {
            stopPolling();
            setPollingState("error");
            setError("Device code expired. Please try again.");
            return;
          }

          try {
            const newAccount = await authApi.authPollForAccount(
              authProvider,
              response.device_code,
            );
            if (newAccount) {
              stopPolling();
              setPollingState("success");
              await refetchStatus();
              await queryClient.invalidateQueries({ queryKey });
              setPollingState("idle");
              setDeviceCode(null);
            }
          } catch (e) {
            const errorMessage = e instanceof Error ? e.message : String(e);
            if (
              !errorMessage.includes("pending") &&
              !errorMessage.includes("slow_down")
            ) {
              stopPolling();
              setPollingState("error");
              setError(errorMessage);
            }
          }
        };

        void pollOnce();
        pollingIntervalRef.current = setInterval(pollOnce, interval);
        pollingTimeoutRef.current = setTimeout(() => {
          stopPolling();
          setPollingState("error");
          setError("Device code expired. Please try again.");
        }, response.expires_in * 1000);
      } catch (e) {
        setPollingState("error");
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [
      authProvider,
      queryClient,
      queryKey,
      refetchStatus,
      startInternalBrowserAuth,
      stopPolling,
    ],
  );

  const cancelAuth = useCallback(async () => {
    stopPolling();
    setPollingState("idle");
    setDeviceCode(null);
    setError(null);
    // 关闭内置浏览器
    if (browserLabelRef.current) {
      try {
        await authApi.closeAuthBrowser(browserLabelRef.current);
      } catch (e) {
        console.debug("[ManagedAuth] Failed to close browser:", e);
      }
      browserLabelRef.current = null;
      setIsInternalBrowserOpen(false);
    }
  }, [stopPolling]);

  const logout = useCallback(() => {
    logoutMutation.mutate();
  }, [logoutMutation]);

  const removeAccount = useCallback(
    (accountId: string) => {
      removeAccountMutation.mutate(accountId);
    },
    [removeAccountMutation],
  );

  const setDefaultAccount = useCallback(
    (accountId: string) => {
      setDefaultAccountMutation.mutate(accountId);
    },
    [setDefaultAccountMutation],
  );

  // 保存凭据
  const saveCredential = useCallback(
    async (username: string, password: string, displayName?: string) => {
      try {
        await authApi.saveAuthCredential(
          authProvider,
          username,
          password,
          displayName,
        );
        await loadCredentials();
      } catch (e) {
        console.error("[ManagedAuth] Failed to save credential:", e);
        throw e;
      }
    },
    [authProvider, loadCredentials],
  );

  // 删除凭据
  const deleteCredential = useCallback(
    async (username: string) => {
      try {
        await authApi.deleteAuthCredential(authProvider, username);
        await loadCredentials();
      } catch (e) {
        console.error("[ManagedAuth] Failed to delete credential:", e);
        throw e;
      }
    },
    [authProvider, loadCredentials],
  );

  const accounts = authStatus?.accounts ?? [];

  return {
    authStatus,
    isLoadingStatus,
    accounts,
    hasAnyAccount: accounts.length > 0,
    isAuthenticated: authStatus?.authenticated ?? false,
    defaultAccountId: authStatus?.default_account_id ?? null,
    migrationError: authStatus?.migration_error ?? null,
    pollingState,
    deviceCode,
    error,
    isPolling: pollingState === "polling",
    isAddingAccount: startLoginMutation.isPending || pollingState === "polling",
    isRemovingAccount: removeAccountMutation.isPending,
    isSettingDefaultAccount: setDefaultAccountMutation.isPending,
    // 内置浏览器相关
    useInternalBrowser,
    isInternalBrowserOpen,
    savedCredentials,
    // 操作
    startAuth,
    startAuthWithCredential,
    addAccount: startAuth,
    cancelAuth,
    logout,
    removeAccount,
    setDefaultAccount,
    saveCredential,
    deleteCredential,
    loadCredentials,
    refetchStatus,
  };
}
