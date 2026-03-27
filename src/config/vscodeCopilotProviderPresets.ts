import type { ProviderCategory, VscodeCopilotProviderConfig } from "@/types";
import type { PresetTheme } from "./claudeProviderPresets";

export interface VscodeCopilotProviderPreset {
  name: string;
  nameKey?: string;
  websiteUrl: string;
  apiKeyUrl?: string;
  settingsConfig: VscodeCopilotProviderConfig;
  isOfficial?: boolean;
  isPartner?: boolean;
  partnerPromotionKey?: string;
  category?: ProviderCategory;
  theme?: PresetTheme;
  icon?: string;
  iconColor?: string;
  isCustomTemplate?: boolean;
}

export const vscodeCopilotProviderPresets: VscodeCopilotProviderPreset[] = [
  {
    name: "Custom Model",
    websiteUrl: "",
    settingsConfig: {
      id: "",
      name: "",
      family: "custom",
      version: "1.0.0",
      maxInputTokens: 128000,
      maxOutputTokens: 8192,
      tooltip: "",
      capabilities: { imageInput: false, toolCalling: true },
      base_url: "",
      api_key: "",
    },
    category: "custom",
    isCustomTemplate: true,
    icon: "copilot",
    iconColor: "#0EA5E9",
  },
];
