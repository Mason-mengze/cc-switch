/**
 * SavedCredentialsDialog - 已保存凭据选择对话框
 *
 * 当用户使用内置浏览器登录且有已保存的账号时，显示此对话框让用户选择账号。
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { User, Plus, Trash2, Key, Eye, EyeOff } from "lucide-react";
import type { CredentialInfo } from "@/lib/api";

interface SavedCredentialsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  credentials: CredentialInfo[];
  onSelect: (username?: string) => void;
  onSave: (username: string, password: string, displayName?: string) => Promise<void>;
  onDelete: (username: string) => Promise<void>;
  isLoading?: boolean;
}

export function SavedCredentialsDialog({
  open,
  onOpenChange,
  credentials,
  onSelect,
  onSave,
  onDelete,
  isLoading,
}: SavedCredentialsDialogProps) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"select" | "add">("select");
  const [newUsername, setNewUsername] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newDisplayName, setNewDisplayName] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [deletingUsername, setDeletingUsername] = useState<string | null>(null);

  const handleSelect = (username?: string) => {
    onSelect(username);
    onOpenChange(false);
  };

  const handleSaveNew = async () => {
    if (!newUsername.trim() || !newPassword.trim()) return;

    setIsSaving(true);
    try {
      await onSave(
        newUsername.trim(),
        newPassword,
        newDisplayName.trim() || undefined,
      );
      // 保存后直接使用这个账号登录
      handleSelect(newUsername.trim());
    } catch (error) {
      console.error("Failed to save credential:", error);
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async (username: string) => {
    setDeletingUsername(username);
    try {
      await onDelete(username);
    } catch (error) {
      console.error("Failed to delete credential:", error);
    } finally {
      setDeletingUsername(null);
    }
  };

  const resetForm = () => {
    setNewUsername("");
    setNewPassword("");
    setNewDisplayName("");
    setShowPassword(false);
    setMode("select");
  };

  // 关闭时重置表单
  const handleOpenChange = (open: boolean) => {
    if (!open) {
      resetForm();
    }
    onOpenChange(open);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {mode === "select"
              ? t("auth.selectAccount", "选择账号")
              : t("auth.addAccount", "添加账号")}
          </DialogTitle>
          <DialogDescription>
            {mode === "select"
              ? t(
                  "auth.selectAccountDescription",
                  "选择一个已保存的账号进行登录，或添加新账号",
                )
              : t(
                  "auth.addAccountDescription",
                  "输入 GitHub 账号和密码，将在登录时自动填充",
                )}
          </DialogDescription>
        </DialogHeader>

        {mode === "select" ? (
          <>
            <ScrollArea className="max-h-[300px] pr-4">
              <div className="space-y-2">
                {/* 不使用预填充选项 */}
                <button
                  type="button"
                  onClick={() => handleSelect(undefined)}
                  disabled={isLoading}
                  className="w-full flex items-center gap-3 p-3 rounded-lg border border-border hover:bg-muted/50 transition-colors text-left"
                >
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-muted">
                    <User className="h-5 w-5 text-muted-foreground" />
                  </div>
                  <div className="flex-1">
                    <p className="font-medium">
                      {t("auth.loginWithoutSaved", "不使用已保存的账号")}
                    </p>
                    <p className="text-sm text-muted-foreground">
                      {t("auth.manualLogin", "手动输入账号密码")}
                    </p>
                  </div>
                </button>

                {/* 已保存的账号列表 */}
                {credentials.map((cred) => (
                  <div
                    key={cred.username}
                    className="flex items-center gap-2 p-3 rounded-lg border border-border hover:bg-muted/50 transition-colors"
                  >
                    <button
                      type="button"
                      onClick={() => handleSelect(cred.username)}
                      disabled={isLoading || deletingUsername === cred.username}
                      className="flex-1 flex items-center gap-3 text-left"
                    >
                      <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10">
                        <Key className="h-5 w-5 text-primary" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="font-medium truncate">
                          {cred.display_name || cred.username}
                        </p>
                        {cred.display_name && (
                          <p className="text-sm text-muted-foreground truncate">
                            {cred.username}
                          </p>
                        )}
                        {cred.last_used_at && (
                          <p className="text-xs text-muted-foreground">
                            {t("auth.lastUsed", "上次使用")}: {new Date(cred.last_used_at).toLocaleDateString()}
                          </p>
                        )}
                      </div>
                    </button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted-foreground hover:text-red-500"
                      onClick={() => handleDelete(cred.username)}
                      disabled={deletingUsername === cred.username}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                ))}
              </div>
            </ScrollArea>

            <DialogFooter className="flex-row gap-2 sm:justify-between">
              <Button
                type="button"
                variant="outline"
                onClick={() => setMode("add")}
              >
                <Plus className="mr-2 h-4 w-4" />
                {t("auth.addNewAccount", "添加新账号")}
              </Button>
              <Button
                type="button"
                variant="ghost"
                onClick={() => onOpenChange(false)}
              >
                {t("common.cancel", "取消")}
              </Button>
            </DialogFooter>
          </>
        ) : (
          <>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="username">
                  {t("auth.username", "用户名/邮箱")}
                </Label>
                <Input
                  id="username"
                  type="text"
                  value={newUsername}
                  onChange={(e) => setNewUsername(e.target.value)}
                  placeholder="username@example.com"
                  autoComplete="username"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="password">{t("auth.password", "密码")}</Label>
                <div className="relative">
                  <Input
                    id="password"
                    type={showPassword ? "text" : "password"}
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    placeholder="••••••••"
                    autoComplete="current-password"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0 top-0 h-full px-3"
                    onClick={() => setShowPassword(!showPassword)}
                  >
                    {showPassword ? (
                      <EyeOff className="h-4 w-4" />
                    ) : (
                      <Eye className="h-4 w-4" />
                    )}
                  </Button>
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="displayName">
                  {t("auth.displayName", "显示名称")}
                  <span className="text-muted-foreground ml-1">
                    ({t("common.optional", "可选")})
                  </span>
                </Label>
                <Input
                  id="displayName"
                  type="text"
                  value={newDisplayName}
                  onChange={(e) => setNewDisplayName(e.target.value)}
                  placeholder={t("auth.displayNamePlaceholder", "工作账号")}
                />
              </div>

              <p className="text-xs text-muted-foreground">
                {t(
                  "auth.passwordStorageNote",
                  "密码将使用 AES-256 加密存储在本地，仅用于自动填充登录表单。",
                )}
              </p>
            </div>

            <DialogFooter className="flex-row gap-2 sm:justify-between">
              <Button
                type="button"
                variant="ghost"
                onClick={() => setMode("select")}
              >
                {t("common.back", "返回")}
              </Button>
              <Button
                type="button"
                onClick={handleSaveNew}
                disabled={!newUsername.trim() || !newPassword.trim() || isSaving}
              >
                {isSaving
                  ? t("common.saving", "保存中...")
                  : t("auth.saveAndLogin", "保存并登录")}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
