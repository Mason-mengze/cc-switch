import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { FormLabel } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ApiKeySection } from "./shared";
import type { ProviderCategory, VscodeCopilotProviderConfig } from "@/types";

interface VscodeCopilotFormFieldsProps {
  config: string;
  onConfigChange: (value: string) => void;
  category?: ProviderCategory;
  shouldShowApiKeyLink: boolean;
  websiteUrl: string;
  isPartner?: boolean;
  partnerPromotionKey?: string;
}

const DEFAULT_CONFIG: VscodeCopilotProviderConfig = {
  id: "",
  name: "",
  family: "custom",
  version: "1.0.0",
  maxInputTokens: 128000,
  maxOutputTokens: 8192,
  tooltip: "",
  capabilities: {
    imageInput: false,
    toolCalling: true,
  },
  base_url: "",
  api_key: "",
};

const FAMILY_OPTIONS = [
  "claude",
  "openai",
  "gemini",
  "deepseek",
  "custom",
] as const;

export function VscodeCopilotFormFields({
  config,
  onConfigChange,
  category,
  shouldShowApiKeyLink,
  websiteUrl,
  isPartner,
  partnerPromotionKey,
}: VscodeCopilotFormFieldsProps) {
  const { t } = useTranslation();

  const parsedConfig = useMemo(() => {
    try {
      const raw = JSON.parse(config || "{}") as VscodeCopilotProviderConfig;
      return {
        ...DEFAULT_CONFIG,
        ...raw,
        capabilities: {
          ...DEFAULT_CONFIG.capabilities,
          ...raw.capabilities,
        },
      };
    } catch {
      return DEFAULT_CONFIG;
    }
  }, [config]);

  const updateConfig = (
    patch: Partial<VscodeCopilotProviderConfig>,
    nestedCapabilities?: Partial<
      NonNullable<VscodeCopilotProviderConfig["capabilities"]>
    >,
  ) => {
    const next: VscodeCopilotProviderConfig = {
      ...parsedConfig,
      ...patch,
      capabilities: {
        ...parsedConfig.capabilities,
        ...nestedCapabilities,
      },
    };
    onConfigChange(JSON.stringify(next, null, 2));
  };

  return (
    <>
      <div className="space-y-2">
        <FormLabel htmlFor="vscode-model-id">
          {t("vscodeCopilot.modelId", { defaultValue: "模型 ID" })}
        </FormLabel>
        <Input
          id="vscode-model-id"
          value={parsedConfig.id}
          onChange={(e) => updateConfig({ id: e.target.value })}
          placeholder="claude-sonnet-4-20250514"
        />
      </div>

      <div className="space-y-2">
        <FormLabel htmlFor="vscode-model-name">
          {t("vscodeCopilot.displayName", { defaultValue: "显示名称" })}
        </FormLabel>
        <Input
          id="vscode-model-name"
          value={parsedConfig.name}
          onChange={(e) => updateConfig({ name: e.target.value })}
          placeholder="Claude Sonnet 4"
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-2">
          <FormLabel>
            {t("vscodeCopilot.family", { defaultValue: "模型家族" })}
          </FormLabel>
          <Select
            value={parsedConfig.family}
            onValueChange={(value) => updateConfig({ family: value })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FAMILY_OPTIONS.map((family) => (
                <SelectItem key={family} value={family}>
                  {family}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <FormLabel htmlFor="vscode-model-version">
            {t("vscodeCopilot.version", { defaultValue: "版本号" })}
          </FormLabel>
          <Input
            id="vscode-model-version"
            value={parsedConfig.version ?? ""}
            onChange={(e) => updateConfig({ version: e.target.value })}
            placeholder="1.0.0"
          />
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-2">
          <FormLabel htmlFor="vscode-max-input">
            {t("vscodeCopilot.maxInputTokens", {
              defaultValue: "最大输入 Tokens",
            })}
          </FormLabel>
          <Input
            id="vscode-max-input"
            type="number"
            value={parsedConfig.maxInputTokens ?? ""}
            onChange={(e) =>
              updateConfig({
                maxInputTokens: e.target.value
                  ? parseInt(e.target.value, 10)
                  : undefined,
              })
            }
            placeholder="128000"
          />
        </div>

        <div className="space-y-2">
          <FormLabel htmlFor="vscode-max-output">
            {t("vscodeCopilot.maxOutputTokens", {
              defaultValue: "最大输出 Tokens",
            })}
          </FormLabel>
          <Input
            id="vscode-max-output"
            type="number"
            value={parsedConfig.maxOutputTokens ?? ""}
            onChange={(e) =>
              updateConfig({
                maxOutputTokens: e.target.value
                  ? parseInt(e.target.value, 10)
                  : undefined,
              })
            }
            placeholder="8192"
          />
        </div>
      </div>

      <div className="space-y-2">
        <FormLabel htmlFor="vscode-tooltip">
          {t("vscodeCopilot.tooltip", { defaultValue: "提示文案" })}
        </FormLabel>
        <Input
          id="vscode-tooltip"
          value={parsedConfig.tooltip ?? ""}
          onChange={(e) => updateConfig({ tooltip: e.target.value })}
          placeholder="Claude Sonnet 4 via CC Switch"
        />
      </div>

      <div className="space-y-2">
        <FormLabel htmlFor="vscode-base-url">
          {t("vscodeCopilot.apiBaseUrl", { defaultValue: "API Base URL" })}
        </FormLabel>
        <Input
          id="vscode-base-url"
          value={parsedConfig.base_url ?? ""}
          onChange={(e) =>
            updateConfig({ base_url: e.target.value.trim().replace(/\/+$/, "") })
          }
          placeholder="https://api.example.com/v1"
        />
      </div>

      <ApiKeySection
        value={parsedConfig.api_key ?? ""}
        onChange={(value) => updateConfig({ api_key: value })}
        category={category}
        shouldShowLink={shouldShowApiKeyLink}
        websiteUrl={websiteUrl}
        isPartner={isPartner}
        partnerPromotionKey={partnerPromotionKey}
      />

      <div className="flex items-center justify-between rounded-lg border border-border/60 p-3">
        <div className="space-y-0.5">
          <FormLabel>
            {t("vscodeCopilot.imageInput", { defaultValue: "支持图片输入" })}
          </FormLabel>
          <p className="text-xs text-muted-foreground">
            {t("vscodeCopilot.imageInputHint", {
              defaultValue: "开启后，扩展会向 VS Code 声明该模型支持图片输入。",
            })}
          </p>
        </div>
        <Switch
          checked={parsedConfig.capabilities?.imageInput ?? false}
          onCheckedChange={(checked) =>
            updateConfig({}, { imageInput: checked })
          }
        />
      </div>

      <div className="flex items-center justify-between rounded-lg border border-border/60 p-3">
        <div className="space-y-0.5">
          <FormLabel>
            {t("vscodeCopilot.toolCalling", { defaultValue: "支持工具调用" })}
          </FormLabel>
          <p className="text-xs text-muted-foreground">
            {t("vscodeCopilot.toolCallingHint", {
              defaultValue: "关闭后，扩展会把该模型声明为不支持工具调用。",
            })}
          </p>
        </div>
        <Switch
          checked={parsedConfig.capabilities?.toolCalling ?? true}
          onCheckedChange={(checked) =>
            updateConfig({}, { toolCalling: checked })
          }
        />
      </div>
    </>
  );
}
