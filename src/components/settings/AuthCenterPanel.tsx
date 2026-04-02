import { Github, ShieldCheck, Globe } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { CopilotAuthSection } from "@/components/providers/forms/CopilotAuthSection";
import { useSettings } from "@/hooks/useSettings";

export function AuthCenterPanel() {
  const { t } = useTranslation();
  const { settings, autoSaveSettings } = useSettings();

  const handleInternalBrowserToggle = async (checked: boolean) => {
    await autoSaveSettings({ useInternalBrowser: checked });
  };

  return (
    <div className="space-y-6">
      <section className="rounded-xl border border-border/60 bg-card/60 p-6">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <ShieldCheck className="h-5 w-5 text-primary" />
              <h3 className="text-base font-semibold">
                {t("settings.authCenter.title", {
                  defaultValue: "OAuth 认证中心",
                })}
              </h3>
            </div>
            <p className="text-sm text-muted-foreground">
              {t("settings.authCenter.description", {
                defaultValue:
                  "集中管理跨应用复用的 OAuth 账号。Provider 只绑定这些认证源，不再重复登录。",
              })}
            </p>
          </div>
          <Badge variant="secondary">
            {t("settings.authCenter.beta", { defaultValue: "Beta" })}
          </Badge>
        </div>
      </section>

      {/* 登录方式设置 */}
      <section className="rounded-xl border border-border/60 bg-card/60 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted">
            <Globe className="h-5 w-5" />
          </div>
          <div className="flex-1">
            <h4 className="font-medium">
              {t("settings.authCenter.loginMethod.title", {
                defaultValue: "登录方式",
              })}
            </h4>
            <p className="text-sm text-muted-foreground">
              {t("settings.authCenter.loginMethod.description", {
                defaultValue: "选择 OAuth 授权时使用的浏览器",
              })}
            </p>
          </div>
        </div>

        <div className="flex items-center justify-between rounded-lg border border-border/40 bg-muted/30 p-4">
          <div className="space-y-1">
            <Label htmlFor="internal-browser" className="font-medium">
              {t("settings.authCenter.loginMethod.internalBrowser", {
                defaultValue: "使用内置浏览器",
              })}
            </Label>
            <p className="text-xs text-muted-foreground">
              {t("settings.authCenter.loginMethod.internalBrowserHint", {
                defaultValue:
                  "在应用内打开登录页面，支持账号密码记忆。关闭则使用系统默认浏览器。",
              })}
            </p>
          </div>
          <Switch
            id="internal-browser"
            checked={settings?.useInternalBrowser ?? false}
            onCheckedChange={handleInternalBrowserToggle}
          />
        </div>
      </section>

      <section className="rounded-xl border border-border/60 bg-card/60 p-6">
        <div className="mb-4 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted">
            <Github className="h-5 w-5" />
          </div>
          <div>
            <h4 className="font-medium">GitHub Copilot</h4>
            <p className="text-sm text-muted-foreground">
              {t("settings.authCenter.copilotDescription", {
                defaultValue:
                  "管理 GitHub Copilot 账号、默认账号以及供 Claude / Codex / Gemini 绑定的托管凭据。",
              })}
            </p>
          </div>
        </div>

        <CopilotAuthSection />
      </section>
    </div>
  );
}
